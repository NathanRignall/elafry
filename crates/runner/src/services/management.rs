use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use command_fds::{CommandFdExt, FdMapping};

use crate::services::communication::Endpoint;
use crate::services::scheduler::{MajorFrame, MinorFrame};
use crate::Component;

use super::communication::RouteEndpoint;
use super::scheduler::Schedule;

enum State {
    Idle,
    Loading {
        configuration: String,
    },
    Running {
        current_task: usize,
        tasks: Vec<elafry::types::configuration::Task>,
        blocked: bool,
        action_status: HashMap<uuid::Uuid, Arc<Mutex<ActionState>>>,
    },
}

#[derive(PartialEq, Debug)]
enum ActionState {
    Started,
    Running,
    Stopped,
    Completed,
}

pub struct ManagementService {
    state: State,
    background: std::thread::JoinHandle<()>,
}

impl ManagementService {
    pub fn new(configuration: String) -> ManagementService {
        ManagementService {
            state: State::Loading {
                configuration: configuration,
            },
            background: std::thread::spawn(|| {}),
        }
    }

    pub fn run(&mut self, state: &mut crate::GlobalState) {
        // get exit message
        if state.messages.contains_key(&0) {
            let messages = state.messages.get_mut(&0).unwrap();

            match messages.pop() {
                Some(_) => {
                    // kill runner
                    log::info!("Received kill message");
                    state.done = true;
                }
                None => {}
            }
        }

        // run the management service state machine
        match &mut self.state {
            State::Idle => {
                // check if there are any requests to change the configuration
                if state.messages.contains_key(&1) {
                    let messages = state.messages.get_mut(&1).unwrap();

                    match messages.pop() {
                        Some(message) => {
                            log::debug!("Received message: {:?}", message);

                            // convert message to string
                            let message = std::str::from_utf8(&message.data).unwrap();
                            self.state = State::Loading {
                                configuration: message.to_string(),
                            };
                        }
                        None => {}
                    }
                }
            }
            State::Loading { configuration } => {
                log::debug!("State Loading");

                // read the configuration file
                let file = std::fs::File::open(configuration).unwrap();
                let configuration: Result<
                    elafry::types::configuration::Configuration,
                    serde_yaml::Error,
                > = serde_yaml::from_reader(file);

                // check if the configuration file was read successfully
                match configuration {
                    Ok(configuration) => {
                        log::debug!("Configuration file read successfully");

                        // check if the configuration has any tasks
                        if configuration.tasks.is_empty() {
                            log::error!("No tasks in configuration file");
                            self.state = State::Idle;
                        } else {
                            self.state = State::Running {
                                current_task: 0,
                                tasks: configuration.tasks,
                                blocked: false,
                                action_status: HashMap::new(),
                            };
                        }
                    }
                    Err(error) => {
                        log::error!("Error reading configuration file: {}", error);
                        self.state = State::Idle;
                    }
                }
            }
            State::Running {
                current_task,
                tasks,
                blocked,
                action_status,
            } => {
                log::debug!("State Running");

                // get actions from first task
                let actions = tasks[*current_task].actions.clone();

                // execute actions
                match actions {
                    elafry::types::configuration::Action::Blocking(actions) => {
                        if *blocked {
                            log::warn!("Non-blocking actions already running");
                        } else {
                            for action in actions {
                                execute_blocking(state, action.data);
                            }
                        }
                    }
                    elafry::types::configuration::Action::NonBlocking(actions) => {
                        if *blocked {
                            for action in actions {
                                let status = action_status.get(&action.id).unwrap();
                                execute_non_blocking(state, status, action.data);
                            }

                            // if all non-blocking actions are done, set blocked to false
                            if action_status.values().all(|status| {
                                let status = status.lock().unwrap();
                                *status == ActionState::Completed
                            }) {
                                log::debug!("All non-blocking actions done");
                                *blocked = false;
                            }
                        } else {
                            for action in actions {
                                action_status
                                    .insert(action.id, Arc::new(Mutex::new(ActionState::Started)));
                                *blocked = true;
                            }
                        }
                    }
                }

                // check if non-blocking actions are done
                if !*blocked {
                    // check if there are more tasks
                    if *current_task < tasks.len() - 1 {
                        self.state = State::Running {
                            current_task: *current_task + 1,
                            tasks: tasks.clone(),
                            blocked: blocked.clone(),
                            action_status: action_status.clone(),
                        };
                    } else {
                        self.state = State::Idle;
                    }
                }
            }
        }
    }
}

fn start_component(state: &mut crate::GlobalState, id: uuid::Uuid) {
    log::debug!("Starting component {}", id);

    // get the component
    match state.components.get_mut(&id) {
        Some(component) => {
            log::debug!("Got component {}", id);

            // don't start if not finish initializing
            if component.implentation.is_none() {
                log::error!("Component {} not initialized", id);
                return;
            }

            // start the component
            component.run = true;

            log::debug!("Started component {}", id);
        }
        None => {
            panic!("Component {} not found", id)
        }
    }
}

fn stop_component(state: &mut crate::GlobalState, id: uuid::Uuid) {
    log::debug!("Stopping component {}", id);

    // get the component
    match state.components.get_mut(&id) {
        Some(component) => {
            log::debug!("Got component {}", id);

            // stop the component
            component.run = false;

            log::debug!("Stopped component {}", id);
        }
        None => {
            panic!("Component {} not found", id)
        }
    }
}

fn add_route(state: &mut crate::GlobalState, source: RouteEndpoint, target: RouteEndpoint) {
    log::debug!("Adding route from {:?} to {:?}", source, target);

    // add the route to the state
    state.routes.insert(source, target);

    log::debug!("Added route");
}

fn remove_route(state: &mut crate::GlobalState, from: RouteEndpoint) {
    log::debug!("Removing route from {:?}", from);

    // remove the route from the state
    state.routes.remove(&from);

    log::debug!("Removed route from {:?}", from);
}

fn set_schedule(state: &mut crate::GlobalState, schedule: Schedule) {
    log::debug!("Setting schedule");

    // check if the schedule is valid by checking if all components in the schedule are in the state and initialized
    for major_frame in &schedule.major_frames {
        for minor_frame in &major_frame.minor_frames {
            match state.components.get(&minor_frame.component_id) {
                Some(component) => {
                    if component.implentation.is_none() {
                        log::error!("Component {} not initialized", minor_frame.component_id);
                        return;
                    }
                }
                None => {
                    log::error!("Component {} not found", minor_frame.component_id);
                    return;
                }
            }
        }
    }

    // set the schedule in the state
    state.schedule = schedule;

    log::debug!("Set schedule");
}

pub fn execute_blocking(
    state: &mut crate::GlobalState,
    data: elafry::types::configuration::BlockingData,
) {
    match data {
        elafry::types::configuration::BlockingData::StartComponent(data) => {
            start_component(state, data.component_id);
        }
        elafry::types::configuration::BlockingData::StopComponent(data) => {
            stop_component(state, data.component_id);
        }
        elafry::types::configuration::BlockingData::AddRoute(data) => {
            add_route(
                state,
                RouteEndpoint {
                    endpoint: match &data.source.endpoint {
                        elafry::types::configuration::Endpoint::Component(endpoint) => {
                            Endpoint::Component(endpoint.clone())
                        }
                        elafry::types::configuration::Endpoint::Address(endpoint) => {
                            Endpoint::Address(endpoint.parse().unwrap())
                        }
                        elafry::types::configuration::Endpoint::Runner => Endpoint::Runner,
                    },
                    channel_id: data.source.channel_id,
                },
                RouteEndpoint {
                    endpoint: match &data.target.endpoint {
                        elafry::types::configuration::Endpoint::Component(endpoint) => {
                            Endpoint::Component(endpoint.clone())
                        }
                        elafry::types::configuration::Endpoint::Address(endpoint) => {
                            Endpoint::Address(endpoint.clone().parse().unwrap())
                        }
                        elafry::types::configuration::Endpoint::Runner => Endpoint::Runner,
                    },
                    channel_id: data.target.channel_id,
                },
            );
        }
        elafry::types::configuration::BlockingData::RemoveRoute(data) => {
            remove_route(
                state,
                RouteEndpoint {
                    endpoint: match &data.source.endpoint {
                        elafry::types::configuration::Endpoint::Component(endpoint) => {
                            Endpoint::Component(endpoint.clone())
                        }
                        elafry::types::configuration::Endpoint::Address(endpoint) => {
                            Endpoint::Address(endpoint.parse().unwrap())
                        }
                        elafry::types::configuration::Endpoint::Runner => Endpoint::Runner,
                    },
                    channel_id: data.source.channel_id,
                },
            );
        }
        elafry::types::configuration::BlockingData::SetSchedule(data) => {
            set_schedule(
                state,
                Schedule {
                    period: std::time::Duration::from_micros(1_000_000 / data.frequency),
                    major_frames: data
                        .major_frames
                        .into_iter()
                        .map(|frame| MajorFrame {
                            minor_frames: frame
                                .minor_frames
                                .into_iter()
                                .map(|frame| MinorFrame {
                                    component_id: frame.component_id,
                                })
                                .collect(),
                        })
                        .collect(),
                },
            );
        }
    }
}

fn add_component(
    state: &mut crate::GlobalState,
    status: &Arc<Mutex<ActionState>>,
    id: uuid::Uuid,
    path: &str,
    core: usize,
) {
    // get a lock on the status
    let mut status_lock = status.lock().unwrap();

    log::debug!("Adding component {} {:?}", id, *status_lock);

    match *status_lock {
        ActionState::Started => {
            // create the component
            let component = Component {
                run: false,
                remove: false,
                path: path.to_string(),
                core,
                implentation: None,
                times: Vec::new(),
            };

            // add the component to the state
            state.components.insert(id, component);

            // set the status to running
            *status_lock = ActionState::Running;
        }
        ActionState::Running => {
            // do something
            log::warn!("Do something (add_component)");

            // set the status to done
            *status_lock = ActionState::Stopped;
        }
        ActionState::Stopped => {
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

            // wait for the component to be ready
            let mut buffer = [0; 1];
            control_socket.read_exact(&mut buffer).unwrap();
            if buffer[0] != b'k' {
                panic!("Failed to start component");
            }

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

            // put the implementation in the component
            match state.components.get_mut(&id) {
                Some(component) => {
                    component.implentation = Some(implementation);
                }
                None => {
                    panic!("Component {} not found", id)
                }
            }

            // set the status to completed
            *status_lock = ActionState::Completed;
        }
        ActionState::Completed => {
            log::warn!("Should not be here");
        }
    }

    log::debug!("Created component {}", id);

    log::debug!("Added component {}", id);
}

fn remove_component(
    state: &mut crate::GlobalState,
    status: &Arc<Mutex<ActionState>>,
    id: uuid::Uuid,
) {
    // get a lock on the status
    let mut status_lock = status.lock().unwrap();

    log::debug!("Removing component {} {:?}", id, *status_lock);

    match *status_lock {
        ActionState::Started => {
            // get the component
            match state.components.get_mut(&id) {
                Some(component) => {
                    // flag the component for removal
                    component.remove = true;
                    component.run = false;

                    // set the status to running
                    *status_lock = ActionState::Running;
                }
                None => {
                    panic!("Component {} not found", id)
                }
            }
        }
        ActionState::Running => {
            // do something
            log::warn!("Do something (add_component)");

            // set the status to done
            *status_lock = ActionState::Stopped;
        }
        ActionState::Stopped => {
            // get the component
            match state.components.get_mut(&id) {
                Some(component) => {
                    // stop the component

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

                            // mark the component as not running
                            component.implentation = None;
                            state.components.remove(&id);

                            // set the status to completed
                            *status_lock = ActionState::Completed;
                        }
                        None => {
                            panic!("Component not started {:?}", component.path);
                        }
                    }
                }
                None => {
                    panic!("Component {} not found", id)
                }
            }

            *status_lock = ActionState::Completed;
        }
        ActionState::Completed => {
            log::warn!("Should not be here");
        }
    }
}

fn execute_non_blocking(
    state: &mut crate::GlobalState,
    status: &Arc<Mutex<ActionState>>,
    data: elafry::types::configuration::NonBlockingData,
) {
    match data {
        elafry::types::configuration::NonBlockingData::AddComponent(data) => {
            add_component(state, status, data.component_id, &data.component, data.core);
        }
        elafry::types::configuration::NonBlockingData::RemoveComponent(data) => {
            remove_component(state, status, data.component_id);
        }
    }
}
