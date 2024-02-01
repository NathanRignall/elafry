use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::sync::{Mutex, RwLock};

use wrapper::communications::Message;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct Address {
    pub app_id: u32,
    pub channel_id: u32,
}

pub struct Router {
    listeners: HashMap<u32, mpsc::Sender<()>>,
    routes: Arc<RwLock<HashMap<Address, Address>>>,
    writers: Arc<RwLock<HashMap<u32, Arc<Mutex<OwnedWriteHalf>>>>>,
}

impl Router {
    pub fn new () -> Router {
        Router {
            listeners: HashMap::new(),
            routes: Arc::new(RwLock::new(HashMap::new())),
            writers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_listener(&mut self, id: u32) {
        let path = format!("/tmp/sock-{}", id);

        println!("Adding listener: {}", path);

        // delete the socket file if it already exists
        if std::path::Path::new(&path).exists() {
            std::fs::remove_file(&path).unwrap();
        }

        // create a new listener
        let (tx, mut rx) = mpsc::channel(1);
        let listener = UnixListener::bind(path.clone()).unwrap();
        self.listeners.insert(id, tx);

        // create a new state
        let connection_closed = Arc::new(Mutex::new(false));

        // clone the state
        let routes_clone = Arc::clone(&self.routes);
        let writers_clone = Arc::clone(&self.writers);

        tokio::spawn(async move {
            println!("Listening on: {}", path);
            loop {
                tokio::select! {
                    Ok((stream, _)) = listener.accept() => {
                        println!("Accepted connection on: {}", id);

                        let connection_closed_clone = Arc::clone(&connection_closed);

                        // clone the state
                        let nested_routes_clone = Arc::clone(&routes_clone);
                        let nested_writers_clone = Arc::clone(&writers_clone);

                        tokio::spawn(async move {

                            println!("Handling connection on: {}", id);

                            // get a reader and add the writer to the writers
                            let mut reader = {
                                let mut writers_lock = nested_writers_clone.write().await;
                                let (reader, writer) = stream.into_split();
                                writers_lock.insert(id, Arc::new(Mutex::new(writer)));
                                reader
                            };

                            // loop until the connection is closed
                            loop {
                                if *connection_closed_clone.lock().await {
                                    println!("X1 Connection closed by controller on: {}", id);
                                    break;
                                }

                                let mut source_length_buf = [0; 4];

                                let read_future = reader.read_exact(&mut source_length_buf);

                                tokio::select! {
                                    result = read_future => {
                                        match result {
                                            Ok(_) => {
                                                // get the length of the message
                                                let length = u32::from_be_bytes(source_length_buf) as usize;

                                                // read the message
                                                let mut source_message_buf = vec![0; length];
                                                reader.read_exact(&mut source_message_buf).await.unwrap();

                                                // deserialize the message
                                                let source_message: Message = match bincode::deserialize(&source_message_buf) {
                                                    Ok(message) => message,
                                                    Err(e) => {
                                                        println!("Failed to deserialize message; err = {:?}", e);
                                                        continue;
                                                    }
                                                };

                                                // find the destination address
                                                let nested_routes_lock = nested_routes_clone.read().await;
                                                let destination: Option<Address>;
                                                {
                                                    destination = nested_routes_lock.get(&Address { app_id: id, channel_id: source_message.channel_id }).cloned();
                                                }

                                                match destination {
                                                    Some(destination) => {
                                                        // get the writer
                                                        let writers_lock = nested_writers_clone.read().await;
                                                        let writer = writers_lock.get(&destination.app_id).cloned();

                                                        match writer {
                                                            Some(writer) => {
                                                                // create a new message
                                                                let destination_message = Message {
                                                                    channel_id: destination.channel_id,
                                                                    data: source_message.data,
                                                                    count: source_message.count,
                                                                    timestamp: source_message.timestamp,
                                                                };

                                                                // serialize the message
                                                                let destination_message_buf = bincode::serialize(&destination_message).unwrap();

                                                                // get the length of the message
                                                                let length = destination_message_buf.len();

                                                                // create a buffer for the length
                                                                let mut length = length.to_be_bytes();

                                                                // format final message
                                                                let mut complete_message_buf = Vec::from(length.as_mut());
                                                                complete_message_buf.append(&mut destination_message_buf.clone());

                                                                // get a lock on the writer
                                                                let mut writer_lock = writer.lock().await;

                                                                // try to write the message if failed continue
                                                                match writer_lock.write(&complete_message_buf).await {
                                                                    Ok(_) => {
                                                                        // println!("Forwarded message from: {:?} to: {:?}", Address { app_id: id, channel_id: destination_message.channel_id }, destination)
                                                                    }
                                                                    Err(e) => {
                                                                        println!("Failed to write to socket; err = {:?}", e);
                                                                        continue;
                                                                    }
                                                                };

                                                            }
                                                            None => {
                                                                // println!("No stream found for destination: {:?}", destination);
                                                            }
                                                        }
                                                    }
                                                    None => {
                                                        // println!("No route found for source: {:?}", Address { app_id: id, channel_id: source_message.channel_id });
                                                    }
                                                }
                                            }
                                            Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                                                println!("Connection closed by client on: {}", id);

                                                // remove the writer
                                                let mut writers_lock = nested_writers_clone.write().await;
                                                writers_lock.remove(&id);

                                                break;
                                            }
                                            Err(e) => {
                                                println!("Failed to read from socket; err = {:?}", e);
                                                break;
                                            }
                                        }
                                    }

                                    _ = async {
                                        while !*connection_closed_clone.lock().await {
                                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                        }
                                    } => {
                                        println!("X2 Connection closed by controller on: {}", id);
                                        return;
                                    }
                            }
                        }
                        });
                    }
                    _ = rx.recv() => {
                        *connection_closed.lock().await = true;
                        println!("Closed connection on: {}", path);
                        break;
                    }
                }
            }
            std::fs::remove_file(path).unwrap(); // Clean up the socket file
        });
    }

    pub fn remove_listener(&mut self, id: u32) {
        let path = format!("/tmp/sock-{}", id);

        println!("Removing listener: {}", path);
        if let Some(tx) = self.listeners.remove(&id) {
            println!("Sending close signal to listener: {}", path);
            let _ = tx.send(());
        }
    }

    pub async fn add_route(&mut self, source: Address, destination: Address) {
        println!("Adding route: {:?} -> {:?}", source, destination);
        let mut route_lock = self.routes.write().await;
        route_lock.insert(source, destination);
    }

    pub async fn remove_route(&mut self, source: Address) {
        println!("Removing route: {:?}", source);
        let mut route_lock = self.routes.write().await;
        route_lock.remove(&source);
    }
}