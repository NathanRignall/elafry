use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use uuid::Uuid;

use wrapper::communications::Message;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct Address {
    pub app_id: u32,
    pub channel_id: u32,
}

struct Listener {
    id: u32,
    path: String,
    state: Arc<Mutex<bool>>,
    stream: Option<UnixStream>,
    listener: UnixListener,
}

pub struct Router {
    listeners: Arc<RwLock<HashMap<u32, Listener>>>,
    routes: Arc<RwLock<HashMap<Address, Address>>>,
    exit_buffer: Arc<Mutex<HashMap<u32, Vec<Message>>>>,
}

impl Router {
    pub fn new() -> Router {
        Router {
            listeners: Arc::new(RwLock::new(HashMap::new())),
            routes: Arc::new(RwLock::new(HashMap::new())),
            exit_buffer: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start(&mut self) {
        println!("Router::start");
        // start a thread
        let listeners_clone = Arc::clone(&self.listeners);
        let routes_clone = Arc::clone(&self.routes);
        let exit_buffer_clone = Arc::clone(&self.exit_buffer);

        std::thread::spawn(move || {
            let mut last_time;
            let period = std::time::Duration::from_micros(1_000_000 / 1000 as u64);

            loop {
                last_time = std::time::Instant::now();

                {
                    let mut listeners_lock = listeners_clone.write().unwrap();

                    // check for new connections
                    for (id, listener) in listeners_lock.iter_mut() {
                        if *listener.state.lock().unwrap() {
                            continue;
                        }

                        match listener.listener.accept() {
                            Ok((stream, _)) => {
                                println!("Accepted connection on: {}", id);

                                listener.stream = Some(stream);

                                if let Some(stream) = &listener.stream {
                                    stream.set_nonblocking(true).unwrap();

                                    let mut state_lock = listener.state.lock().unwrap();
                                    *state_lock = true;
                                }
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                            Err(e) => {
                                println!("Failed to accept connection; err = {:?}", e);
                            }
                        }
                    }

                    // check for data
                    for (id, listener) in listeners_lock.iter_mut() {
                        if !*listener.state.lock().unwrap() {
                            continue;
                        }

                        let mut stream = listener.stream.as_ref().unwrap();

                        let mut length_buf = [0; 4];

                        // loop for a maximum of 10 times until no more data is available
                        for _ in 0..1000 {
                            match stream.read_exact(&mut length_buf) {
                                Ok(_) => {
                                    // get length of message
                                    let length = u32::from_be_bytes(length_buf);

                                    // don't read if length is 0
                                    if length == 0 {
                                        continue;
                                    }

                                    // create buffer with length
                                    let mut message_buf = vec![0; length as usize];

                                    // read the message
                                    stream.read_exact(&mut message_buf).unwrap();

                                    // deserialize message
                                    let message: Message = match bincode::deserialize(&message_buf)
                                    {
                                        Ok(message) => message,
                                        Err(e) => {
                                            println!(
                                                "Failed to deserialize message; err = {:?}",
                                                e
                                            );
                                            continue;
                                        }
                                    };

                                    // look for a route
                                    let routes_lock = routes_clone.read().unwrap();

                                    let destination: Option<Address>;
                                    {
                                        destination = routes_lock
                                            .get(&Address {
                                                app_id: *id,
                                                channel_id: message.channel_id,
                                            })
                                            .cloned();
                                    }

                                    match destination {
                                        Some(destination) => {
                                            // get a lock on the exit buffer
                                            let mut exit_buffer_lock =
                                                exit_buffer_clone.lock().unwrap();

                                            // insert the message into the exit buffer
                                            match exit_buffer_lock.get_mut(&destination.app_id) {
                                                Some(buffer) => {
                                                    buffer.push(message);
                                                }
                                                None => {
                                                    exit_buffer_lock
                                                        .insert(destination.app_id, vec![message]);
                                                }
                                            }
                                        }
                                        None => {
                                            // println!("No route found for: {:?}", Address { app_id: *id, channel_id: message.channel_id });
                                        }
                                    }
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    break;
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                                    println!("Connection closed on: {}", id);
                                    let mut state_lock = listener.state.lock().unwrap();
                                    *state_lock = false;
                                    break;
                                }
                                Err(e) => {
                                    println!("Failed to read from socket; err = {:?}", e);
                                    break;
                                }
                            }
                        }
                    }

                    // check for data to send to clear the exit buffer
                    for (id, listener) in listeners_lock.iter_mut() {
                        if !*listener.state.lock().unwrap() {
                            continue;
                        }

                        let mut stream = listener.stream.as_ref().unwrap();

                        let mut exit_buffer_lock = exit_buffer_clone.lock().unwrap();

                        let messages = match exit_buffer_lock.get_mut(&id) {
                            Some(messages) => messages,
                            None => {
                                continue;
                            }
                        };

                        for message in messages.iter() {
                            let message_buf = bincode::serialize(&message).unwrap();

                            let length = message_buf.len() as u32;

                            let length_buf = length.to_be_bytes();

                            let combined_buf = [length_buf.to_vec(), message_buf].concat();

                            match stream.write_all(&combined_buf) {
                                Ok(_) => {}
                                Err(e) => {
                                    println!("Failed to write to socket; err = {:?}", e);
                                    break;
                                }
                            }
                        }

                        exit_buffer_lock.remove(id);
                    }
                }

                let now = std::time::Instant::now();
                let duration = now.duration_since(last_time);

                if duration < period {
                    std::thread::sleep(period - duration);
                } else {
                    println!(
                        "Warning: loop took longer than period {:?}us",
                        duration.as_micros()
                    );
                }
            }
        });
    }

    pub fn add_listener(&mut self, id: u32) {
        let path = format!("/tmp/sock-{}", id);

        println!("Adding listener: {}", path);

        if std::path::Path::new(&path).exists() {
            std::fs::remove_file(&path).unwrap();
        }

        // create a new listener
        let listener = UnixListener::bind(&path).unwrap();

        let mut listeners_lock = self.listeners.write().unwrap();

        let listener = Listener {
            id,
            path,
            state: Arc::new(Mutex::new(false)),
            stream: None,
            listener,
        };

        // set the listener to non-blocking
        listener.listener.set_nonblocking(true).unwrap();

        // insert the listener
        listeners_lock.insert(id, listener);

        println!("Finished adding listener");
    }

    pub fn remove_listener(&mut self, id: u32) {
        let mut listeners_lock = self.listeners.write().unwrap();
        listeners_lock.remove(&id);
    }

    pub fn add_route(&mut self, source: Address, destination: Address) {
        println!("Adding route: {:?} -> {:?}", source, destination);
        let mut route_lock = self.routes.write().unwrap();
        route_lock.insert(source, destination);

        println!("finished adding route");
    }

    pub fn remove_route(&mut self, source: Address) {
        println!("Removing route: {:?}", source);
        let mut route_lock = self.routes.write().unwrap();
        route_lock.remove(&source);

        println!("finished removing route");
    }
}
