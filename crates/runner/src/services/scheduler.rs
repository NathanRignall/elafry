use std::{
    io::{Read, Write},
    os::{
        fd::{FromRawFd, IntoRawFd, OwnedFd},
        unix::net::UnixStream,
    },
    process::{Command, Stdio},
};

use command_fds::{CommandFdExt, FdMapping};

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

pub struct SchedulerService {
    frame_index: usize,
}

impl SchedulerService {
    pub fn new() -> Self {
        SchedulerService { frame_index: 0 }
    }

    fn intialize(&mut self, state: &mut crate::GlobalState) {
        // intialize components that are not implemented
        for (_, component) in state.components.iter_mut() {
            if component.implentation.is_none() && component.remove == false {
                log::debug!("Initializing component {:?}", component.path);

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
                let binary_path = format!("target/release/{}", component.path);
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
                    libc::CPU_SET(component.core, &mut cpu_set);
                    let ret = libc::sched_setaffinity(
                        child.id() as libc::pid_t,
                        std::mem::size_of_val(&cpu_set),
                        &cpu_set,
                    );
                    if ret != 0 {
                        log::error!("Failed to set affinity");
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
                        log::error!("Failed to set scheduler");
                    }
                }

                // // wait for the component to be ready
                // let mut buffer = [0; 1];
                // control_socket.read_exact(&mut buffer).unwrap();
                // if buffer[0] != b'k' {
                //     panic!("Failed to start component");
                // }

                // create the component implementation
                let implementation = crate::Implementation {
                    control_socket: crate::Socket {
                        socket: control_socket,
                        count: 0,
                    },
                    data_socket: crate::Socket {
                        socket: data_socket,
                        count: 0,
                    },
                    state_socket: crate::Socket {
                        socket: state_socket,
                        count: 0,
                    },
                    child,
                };

                component.implentation = Some(implementation);

                log::debug!("Component initialized {:?}", component.path);
            }
        }
    }

    fn execute(&mut self, state: &mut crate::GlobalState) {
        // if there are no major frames, return
        if state.schedule.major_frames.is_empty() {
            log::error!("No major frames");
            return;
        }

        // reset the frame index if it is out of bounds
        if self.frame_index >= state.schedule.major_frames.len() {
            self.frame_index = 0;
        }

        // get the current major frame
        let major_frame = &state.schedule.major_frames[self.frame_index];

        // run the minor frames
        for (_, frame) in major_frame.minor_frames.iter().enumerate() {
            // log::debug!("Running component {:?}", frame.component_id);

            let component = match state.components.get_mut(&frame.component_id) {
                Some(component) => component,
                None => {
                    log::error!("Component not found {:?}", frame.component_id);
                    continue;
                }
            };

            // if the component is not running, continue
            if !component.run {
                log::error!("Component not running {:?}", frame.component_id);
                continue;
            }

            match &mut component.implentation {
                Some(implentation) => {
                    // wake the component
                    implentation
                        .control_socket
                        .socket
                        .write_all(&[b'w', implentation.control_socket.count])
                        .unwrap();
                    implentation.control_socket.count += 1;
                    let mut buffer = [0; 1];
                    implentation
                        .control_socket
                        .socket
                        .read_exact(&mut buffer)
                        .unwrap();

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64;
                    component.times.push(timestamp);

                    // run the component
                    implentation
                        .control_socket
                        .socket
                        .write_all(&[b'r', implentation.control_socket.count])
                        .unwrap();
                    implentation.control_socket.count += 1;
                    let mut buffer = [0; 1];
                    implentation
                        .control_socket
                        .socket
                        .read_exact(&mut buffer)
                        .unwrap();

                    if buffer[0] != b'k' {
                        log::error!("Failed to run component");
                    }
                }
                None => {
                    log::error!("Component not started {:?}", frame.component_id);
                }
            }
        }
    }

    fn remove(&mut self, state: &mut crate::GlobalState) -> Vec<uuid::Uuid> {
        let mut removed = Vec::new();

        for (id, component) in state.components.iter_mut() {
            // if component is marked for removal, remove it
            if component.remove {
                log::debug!("Removing component {:?}", component.path);

                match &mut component.implentation {
                    Some(implentation) => {
                        // wake the component
                        implentation
                            .control_socket
                            .socket
                            .write_all(&[b'w', implentation.control_socket.count])
                            .unwrap();
                        implentation.control_socket.count += 1;
                        let mut buffer = [0; 1];
                        implentation
                            .control_socket
                            .socket
                            .read_exact(&mut buffer)
                            .unwrap();

                        // stop the component
                        implentation
                            .control_socket
                            .socket
                            .write_all(&[b'q', implentation.control_socket.count])
                            .unwrap();
                        
                        // // wait for the component to exit and kill it if it does not
                        // implentation.child.wait().unwrap();
                        // implentation.child.kill().unwrap();
                    }
                    None => {
                        log::error!("Component not started {:?}", component.path);
                    }
                }

                component.implentation = None;
                removed.push(id.clone());
            }
        }

        removed
    }

    fn cleanup(&mut self, state: &mut crate::GlobalState) {
        // get the components that are removed
        let removed = self.remove(state);

        // remove the components from the state
        for id in removed {
            state.components.remove(&id);
        }
    }

    pub fn run(&mut self, state: &mut crate::GlobalState) {
        self.intialize(state);
        self.execute(state);
        self.cleanup(state);
        self.frame_index += 1;
    }
}
