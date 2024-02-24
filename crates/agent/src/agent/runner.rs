use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::spawn;

struct Component {
    id: uuid::Uuid,
    path: String,
    run: bool,
    control_socket: UnixStream,
    child: std::process::Child,
}

pub struct Runner {
    components: Arc<Mutex<HashMap<uuid::Uuid, Component>>>,
}

impl Runner {
    pub fn new() -> Runner {
        Runner {
            components: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add(&mut self, id: uuid::Uuid, path: &str) {
        println!("Adding component {}", path);

        // get lock on the components map
        let mut components = self.components.lock().unwrap();

        // create a pair of sockets
        let (mut control_socket, child_control_socket) = UnixStream::pair().unwrap();

        // spawn the component
        let child = Command::new("target/release/runner")
            .stdin(unsafe { Stdio::from_raw_fd(child_control_socket.into_raw_fd()) }) 
            .spawn()
            .expect("Failed to start component");

        // wait for the child to start
        let mut buffer = [0; 2];
        control_socket.read_exact(&mut buffer).unwrap();
        println!("Component {} is ready", id);

        // write the path and wait for the child to acknowledge
        control_socket.write_all(path.as_bytes()).unwrap();
        control_socket.read_exact(&mut buffer).unwrap();
        println!("Component {} loaded library", id);

        // write the socket path and wait for the child to acknowledge
        let socket_path = format!("/tmp/sock-{}", id);
        control_socket.write_all(socket_path.as_bytes()).unwrap();
        control_socket.read_exact(&mut buffer).unwrap();

        // acknowledge the component init
        control_socket.write_all(b"ok").unwrap();

        // create the component
        let component = Component {
            id,
            path: path.to_owned(),
            run: false,
            control_socket: control_socket,
            child,
        };

        // update the components map
        components.insert(id, component);

        println!("Added component {}", id);
    }

    pub fn start(&mut self, id: uuid::Uuid) {
        println!("Starting component {}", id);

        // update the components map bool flag
        let mut components = self.components.lock().unwrap();
        match components.get_mut(&id) {
            Some(component) => {
                component.run = true;
                println!("Set component {} to run", id);
            }
            None => {}
        }
    }

    pub fn stop(&mut self, id: uuid::Uuid) {
        println!("Stopping component {}", id);

        // update the components map bool flag
        let mut components = self.components.lock().unwrap();
        match components.get_mut(&id) {
            Some(component) => {
                component.run = false;
                println!("Set component {} to stop", id);
            }
            None => {}
        }
    }

    pub fn remove(&mut self, id: uuid::Uuid) {
        println!("Removing component {}", id);

        // kill the child process
        let mut components = self.components.lock().unwrap();
        match components.get_mut(&id) {
            Some(component) => {
                component.child.kill().unwrap();
            }
            None => {}
        }

        // remove the component from the map
        components.remove(&id);

        println!("Removed component {}", id);
    }

    pub fn run(&mut self) {
        let components = self.components.clone();

        spawn(move || {
            let mut last_time;
            let period = std::time::Duration::from_micros(1_000_000 / 200 as u64);

            loop {
                last_time = std::time::Instant::now();

                {
                    let mut components = components.lock().unwrap();
                    for (_, component) in components.iter_mut() {
                        if component.run {
                            component.control_socket.write_all(b"go").unwrap();
                            let mut buffer = [0; 2];
                            component.control_socket.read_exact(&mut buffer).unwrap();
                        }
                    }
                }

                let now = std::time::Instant::now();
                let duration = now.duration_since(last_time);

                if duration < period {
                    std::thread::sleep(period - duration);
                } else {
                    println!("Warning: loop took longer than period {}us", duration.as_micros());
                }
            }
        });
    }
}
