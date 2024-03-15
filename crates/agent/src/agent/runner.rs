use command_fds::{CommandFdExt, FdMapping};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use elafry::types::communication::Message;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct RouteEndpoint {
    pub component_id: uuid::Uuid,
    pub channel_id: u32,
}

pub struct Component {
    run: bool,
    control_socket: UnixStream,
    control_count: u8,
    data_socket: UnixStream,
    data_count: u8,
    state_socket: UnixStream,
    state_count: u8,
    child: std::process::Child,
    times: Vec<u64>,
}

pub struct Schedule {
    pub period: std::time::Duration,
    pub major_frames: Vec<MajorFrame>,
}

pub struct MajorFrame {
    pub minor_frames: Vec<MinorFrame>,
}

pub struct MinorFrame {
    pub component_id: uuid::Uuid,
}

pub struct Runner {
    components: Arc<Mutex<HashMap<uuid::Uuid, Component>>>,
    routes: Arc<Mutex<HashMap<RouteEndpoint, RouteEndpoint>>>,
    schedule: Arc<Mutex<Schedule>>,
    times: Arc<Mutex<Vec<(u64, u64, u64, u64)>>>,
}

impl Runner {
    pub fn new() -> Runner {
        Runner {
            components: Arc::new(Mutex::new(HashMap::new())),
            routes: Arc::new(Mutex::new(HashMap::new())),
            schedule: Arc::new(Mutex::new(Schedule {
                period: std::time::Duration::from_micros(1_000_000 / 100),
                major_frames: Vec::new(),
            })),
            times: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_component(&mut self, id: uuid::Uuid, path: &str, core: usize) {
        println!("Adding component {}", path);

        // create control and data sockets
        let (mut control_socket, child_control_socket) = UnixStream::pair().unwrap();
        let (data_socket, child_data_socket) = UnixStream::pair().unwrap();
        let (state_socket, child_state_socket) = UnixStream::pair().unwrap();
        data_socket.set_nonblocking(true).unwrap();
        state_socket.set_nonblocking(true).unwrap();

        // create fds for the child process
        let child_control_socket_fd = child_control_socket.into_raw_fd();
        let child_data_socket_fd = child_data_socket.into_raw_fd();
        let child_state_socket_fd = child_state_socket.into_raw_fd();

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
                FdMapping {
                    child_fd: 12,
                    parent_fd: unsafe { OwnedFd::from_raw_fd(child_state_socket_fd) },
                },
            ])
            .unwrap();
        // redirect the child's stderr to the parent's stderr
        let child = command
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();

        // stop socket from being closed when it goes out of scope
        let _ = unsafe { UnixStream::from_raw_fd(child_control_socket_fd) };
        let _ = unsafe { UnixStream::from_raw_fd(child_data_socket_fd) };
        let _ = unsafe { UnixStream::from_raw_fd(child_state_socket_fd) };

        // use libc to set the process core affinity to specified core
        let mut cpu_set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::CPU_SET(core, &mut cpu_set);
            let ret = libc::sched_setaffinity(
                child.id() as libc::pid_t,
                std::mem::size_of_val(&cpu_set),
                &cpu_set,
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
                &libc::sched_param { sched_priority: 99 },
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
            data_count: 0,
            state_socket,
            state_count: 0,
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

    pub fn start_component(&mut self, id: uuid::Uuid) {
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

    pub fn stop_component(&mut self, id: uuid::Uuid) {
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

    pub fn remove_component(&mut self, id: uuid::Uuid) {
        println!("Removing component {}", id);

        // kill the child process
        let mut components = self.components.lock().unwrap();
        match components.get_mut(&id) {
            Some(component) => {
                // wake up component
                component
                    .control_socket
                    .write_all(&[b'w', component.control_count])
                    .unwrap();
                component.control_count += 1;
                let mut buffer = [0; 1];
                component.control_socket.read_exact(&mut buffer).unwrap();

                // send stop signal
                component
                    .control_socket
                    .write_all(&[b'q', component.control_count])
                    .unwrap();

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

    pub fn add_route(&mut self, source: RouteEndpoint, target: RouteEndpoint) {
        println!("Adding route: {:?} -> {:?}", source, target);
        let mut route_lock = self.routes.lock().unwrap();
        route_lock.insert(source, target);
        println!("Finished adding route");
    }

    pub fn remove_route(&mut self, source: RouteEndpoint) {
        println!("Removing route: {:?}", source);
        let mut route_lock = self.routes.lock().unwrap();
        route_lock.remove(&source);
        println!("Finished removing route");
    }

    pub fn set_schedule(&mut self, schedule: Schedule) {
        println!("Setting schedule");
        let mut schedule_lock = self.schedule.lock().unwrap();
        *schedule_lock = schedule;
        println!("Finished setting schedule");
    }

    pub fn run(&mut self) {
        let components = self.components.clone();
        let routes = self.routes.clone();
        let schedule = self.schedule.clone();
        let times = self.times.clone();

        let _ = std::thread::spawn(move || {
            let pid = unsafe { libc::getpid() };
            println!("Runner thread pid: {}", pid);

            // use libc to set the process core affinity to core 1
            let mut cpu_set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
            unsafe {
                libc::CPU_SET(1, &mut cpu_set);
                let ret = libc::pthread_setaffinity_np(
                    libc::pthread_self(),
                    std::mem::size_of_val(&cpu_set),
                    &cpu_set,
                );
                if ret != 0 {
                    println!("Failed to set affinity");
                }
            }

            // use libc to set the process sechdeuler to SCHEDULER FFIO
            unsafe {
                let ret = libc::pthread_setschedparam(
                    libc::pthread_self(),
                    libc::SCHED_FIFO,
                    &libc::sched_param { sched_priority: 99 },
                );
                if ret != 0 {
                    println!("Failed to set scheduler");
                }
            }

            // frame index
            let mut index = 0;
            let mut last_sleep = std::time::Duration::from_micros(0);
            let mut last_duration = std::time::Duration::from_micros(0);
            let mut overruns = 0;

            loop {
                let last_time = std::time::Instant::now();

                {
                    // get lock on times
                    let mut times = times.lock().unwrap();

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64;
                    times.push((
                        timestamp,
                        last_sleep.as_micros() as u64,
                        last_duration.as_micros() as u64,
                        overruns,
                    ));
                }

                // get the period and frame count
                let (period, frame_count) = {
                    let schedule = schedule.lock().unwrap();
                    (schedule.period, schedule.major_frames.len())
                };

                // reset index if it's out of bounds
                if index >= frame_count {
                    index = 0;
                }

                // execute components
                if frame_count > 0 {
                    execute_frame(components.clone(), schedule.clone(), index);
                }

                // route messages
                route_messages(components.clone(), routes.clone());

                // increment index for next frame
                index += 1;

                // sleep for the rest of the period
                let now = std::time::Instant::now();
                let duration = now.duration_since(last_time);
                let mut sleep = std::time::Duration::from_micros(0);

                if duration <= period {
                    sleep = period - duration;
                    std::thread::sleep(sleep);
                } else {
                    overruns += 1;
                    println!(
                        "Warning: loop took longer than period {}us - {}us",
                        duration.as_micros(),
                        last_sleep.as_micros()
                    );
                }

                last_duration = duration;
                last_sleep = sleep;
            }
        });
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

        let times = self.times.lock().unwrap();
        let mut writer = csv::Writer::from_path("times.csv").expect("Failed to open file");
        for time in times.iter() {
            writer.serialize(time).expect("Failed to write to file");
        }
    }
}

fn execute_frame(
    components: Arc<Mutex<HashMap<uuid::Uuid, Component>>>,
    schedule: Arc<Mutex<Schedule>>,
    index: usize,
) {
    let schedule = schedule.lock().unwrap();
    let major_frame = &schedule.major_frames[index];

    for (_, frame) in major_frame.minor_frames.iter().enumerate() {
        let mut components = components.lock().unwrap();
        let component = match components.get_mut(&frame.component_id) {
            Some(component) => component,
            None => {
                println!("Component not found {:?}", frame.component_id);
                continue;
            }
        };

        if component.run {
            component
                .control_socket
                .write_all(&[b'w', component.control_count])
                .unwrap();
            component.control_count += 1;
            let mut buffer = [0; 1];
            component.control_socket.read_exact(&mut buffer).unwrap();

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;
            component.times.push(timestamp);

            component
                .control_socket
                .write_all(&[b'r', component.control_count])
                .unwrap();
            component.control_count += 1;
            let mut buffer = [0; 1];
            component.control_socket.read_exact(&mut buffer).unwrap();

            if buffer[0] != b'k' {
                panic!("Failed to run component");
            }
        }
    }
}

fn route_messages(
    components: Arc<Mutex<HashMap<uuid::Uuid, Component>>>,
    routes: Arc<Mutex<HashMap<RouteEndpoint, RouteEndpoint>>>,
) {
    let components_clone = Arc::clone(&components);
    let routes_clone = Arc::clone(&routes);

    let mut components_lock = components_clone.lock().unwrap();
    let routes_lock = routes_clone.lock().unwrap();
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

                    let destination: Option<RouteEndpoint>;
                    {
                        destination = routes_lock
                            .get(&RouteEndpoint {
                                component_id: *id,
                                channel_id: message.channel_id,
                            })
                            .cloned();
                    }

                    match destination {
                        Some(destination) => {
                            // insert the message into the exit buffer
                            match exit_buffer.get_mut(&destination.component_id) {
                                Some(buffer) => {
                                    buffer.push(message);
                                }
                                None => {
                                    exit_buffer.insert(destination.component_id, vec![message]);
                                }
                            }
                        }
                        None => {
                            println!(
                                "No route found for: {:?}",
                                RouteEndpoint {
                                    component_id: *id,
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
