use command_fds::{CommandFdExt, FdMapping};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::thread::JoinHandleExt;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, RwLock};

use elafry::communications::Message;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct Address {
    pub app_id: uuid::Uuid,
    pub channel_id: u32,
}

pub struct Component {
    run: bool,
    control_socket: UnixStream,
    control_count: u8,
    data_socket: UnixStream,
    child: std::process::Child,
    times: Vec<u64>,
}

pub struct Runner {
    components: Arc<Mutex<HashMap<uuid::Uuid, Component>>>,
    routes: Arc<RwLock<HashMap<Address, Address>>>,
}

impl Runner {
    pub fn new() -> Runner {
        Runner {
            components: Arc::new(Mutex::new(HashMap::new())),
            routes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn add(&mut self, id: uuid::Uuid, path: &str) {
        println!("Adding component {}", path);

        // create control and data sockets
        let (mut control_socket, child_control_socket) = UnixStream::pair().unwrap();
        let (data_socket, child_data_socket) = UnixStream::pair().unwrap();
        data_socket.set_nonblocking(true).unwrap();

        // create fds for the child process
        let child_control_socket_fd = child_control_socket.into_raw_fd();
        let child_data_socket_fd = child_data_socket.into_raw_fd();

        // spawn the child process
        let binary_path = format!("target/release/{}", path);
        let mut command = Command::new(binary_path);
        command
            .fd_mappings(vec![
                FdMapping {
                    child_fd: 10,
                    parent_fd: unsafe { OwnedFd::from_raw_fd(child_control_socket_fd) },
                },
                FdMapping {
                    child_fd: 11,
                    parent_fd: unsafe { OwnedFd::from_raw_fd(child_data_socket_fd) },
                },
            ])
            .unwrap();
        // redirect the child's stderr to the parent's stderr
        let child = command
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();

        // use libc to set the process core affinity to core 3
        let mut cpu_set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::CPU_SET(3, &mut cpu_set);
            let ret = libc::sched_setaffinity(
                child.id() as libc::pid_t, 
                std::mem::size_of_val(&cpu_set), 
                &cpu_set
            );
            if ret != 0 {
                panic!("Failed to set affinity");
            }
        }
    
        // use libc to set the process sechdeuler to SCHEDULER FFIO
        unsafe {
            let ret = libc::sched_setscheduler(
                child.id() as libc::pid_t,
                libc::SCHED_FIFO,
                &libc::sched_param {
                    sched_priority: 99,
                },
            );
            if ret != 0 {
                println!("Failed to set scheduler");
            }
        }

        // wait for the component to be ready
        let mut buffer = [0; 1];
        control_socket.read_exact(&mut buffer).unwrap();
        if buffer[0] != b'k' {
            panic!("Failed to start component");
        }

        // create the component
        let component = Component {
            run: false,
            control_socket,
            control_count: 0,
            data_socket,
            child,
            times: Vec::new(),
        };

        println!("Created component {}", id);

        // update the components map
        {
            let mut components = self.components.lock().unwrap();
            components.insert(id, component);
        }

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
            None => {
                panic!("Component not found");
            }
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
            None => {
                panic!("Component not found");
            }
        }
    }

    pub fn remove(&mut self, id: uuid::Uuid) {
        println!("Removing component {}", id);

        // kill the child process
        let mut components = self.components.lock().unwrap();
        match components.get_mut(&id) {
            Some(component) => {
                // wake up component
                component.control_socket.write_all(&[b'w', component.control_count]).unwrap();
                component.control_count += 1;
                let mut buffer = [0; 1];
                component.control_socket.read_exact(&mut buffer).unwrap();

                // send stop signal
                component.control_socket.write_all(&[b'q', component.control_count]).unwrap();

                // wait for the child to exit
                component.child.wait().unwrap();

                // kill
                component.child.kill().unwrap();
            }
            None => {
                panic!("Component not found");
            }
        }

        // remove the component from the map
        components.remove(&id);

        println!("Removed component {}", id);
    }

    pub fn add_route(&mut self, source: Address, destination: Address) {
        println!("Adding route: {:?} -> {:?}", source, destination);
        let mut route_lock = self.routes.write().unwrap();
        route_lock.insert(source, destination);
        println!("Finished adding route");
    }

    pub fn remove_route(&mut self, source: Address) {
        println!("Removing route: {:?}", source);
        let mut route_lock = self.routes.write().unwrap();
        route_lock.remove(&source);
        println!("Finished removing route");
    }

    pub fn run(&mut self) {
        let components = self.components.clone();
        let routes = self.routes.clone();

        let thread = std::thread::spawn(move || {
            let mut last_time;
            let period = std::time::Duration::from_micros(1_000_000 / 200 as u64);

            #[cfg(feature = "instrument")]
            println!("Instrumentation enabled");

            loop {
                last_time = std::time::Instant::now();

                {
                    let mut components = components.lock().unwrap();
                    for (_, component) in components.iter_mut() {
                        if component.run {
                            // wake up component
                            component.control_socket.write_all(&[b'w', component.control_count]).unwrap();
                            component.control_count += 1;
                            let mut buffer = [0; 1];
                            component.control_socket.read_exact(&mut buffer).unwrap();

                            let timestamp = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_micros()
                                as u64;
                            component.times.push(timestamp);

                            // execute component
                            component.control_socket.write_all(&[b'r', component.control_count]).unwrap();
                            component.control_count += 1;
                            let mut buffer = [0; 1];
                            component.control_socket.read_exact(&mut buffer).unwrap();

                            if buffer[0] != b'k' {
                                panic!("Failed to run component");
                            }
                        }
                    }
                }
                {
                    route(components.clone(), routes.clone());
                }

                let now = std::time::Instant::now();
                let duration = now.duration_since(last_time);

                if duration < period {
                    std::thread::sleep(period - duration);
                } else {
                    println!(
                        "Warning: loop took longer than period {}us",
                        duration.as_micros()
                    );
                }
            }
        });

        let pid = unsafe { libc::getpid() };
        println!("Runner thread pid: {}", pid);

        // use libc to set the process core affinity to core 2
        let mut cpu_set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::CPU_SET(2, &mut cpu_set);
            let ret = libc::sched_setaffinity(
                pid as libc::pid_t,
                std::mem::size_of_val(&cpu_set),
                &cpu_set
            );
            if ret != 0 {
                println!("Failed to set affinity");
            }
        }
    
        // use libc to set the process sechdeuler to SCHEDULER FFIO
        unsafe {
            let ret = libc::sched_setscheduler(
                pid as libc::pid_t,
                libc::SCHED_FIFO,
                &libc::sched_param {
                    sched_priority: 99,
                },
            );
            if ret != 0 {
                println!("Failed to set scheduler");
            }
        }
    }

    pub fn write(&mut self) {
        let mut components = self.components.lock().unwrap();
        for (id, component) in components.iter_mut() {
            let instrument_file = format!("instrumentation_{}.csv", id);

            let mut writer = csv::Writer::from_path(instrument_file).expect("Failed to open file");
            for (i, time) in component.times.iter().enumerate() {
                writer
                    .serialize((i, time))
                    .expect("Failed to write to file");
            }
        }
    }
}

fn route(
    components: Arc<Mutex<HashMap<uuid::Uuid, Component>>>,
    routes: Arc<RwLock<HashMap<Address, Address>>>,
) {
    let components_clone = Arc::clone(&components);
    let routes_clone = Arc::clone(&routes);

    let mut components_lock = components_clone.lock().unwrap();
    let mut exit_buffer: HashMap<uuid::Uuid, Vec<Message>> = HashMap::new();

    // check for data
    for (id, listener) in components_lock.iter_mut() {
        let mut length_buf = [0; 4];

        // loop for a maximum of 10 times until no more data is available
        for _ in 0..10 {
            match listener.data_socket.read_exact(&mut length_buf) {
                Ok(_) => {
                    // get length of message
                    let length = u32::from_be_bytes(length_buf);

                    // don't read if length is 0
                    if length == 0 {
                        continue;
                    }

                    // create buffer with length
                    let message_buf = {
                        let mut buf = vec![0; length as usize];
                        match listener.data_socket.read_exact(&mut buf) {
                            Ok(_) => buf,
                            Err(e) => {
                                println!("Failed to read from socket; err = {:?}", e);
                                continue;
                            }
                        }
                    };

                    // deserialize message
                    let message: Message = match bincode::deserialize(&message_buf) {
                        Ok(message) => message,
                        Err(e) => {
                            println!("Failed to deserialize message; err = {:?}", e);
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
                            // insert the message into the exit buffer
                            match exit_buffer.get_mut(&destination.app_id) {
                                Some(buffer) => {
                                    buffer.push(message);
                                }
                                None => {
                                    exit_buffer.insert(destination.app_id, vec![message]);
                                }
                            }
                        }
                        None => {
                            println!(
                                "No route found for: {:?}",
                                Address {
                                    app_id: *id,
                                    channel_id: message.channel_id
                                }
                            );
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
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
    for (id, listener) in components_lock.iter_mut() {
        let messages = match exit_buffer.get_mut(&id) {
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

            match listener.data_socket.write_all(&combined_buf) {
                Ok(_) => {}
                Err(e) => {
                    println!("Failed to write to socket; err = {:?} {:?}", e, id);
                    break;
                }
            }
        }

        exit_buffer.remove(id);
    }
}
