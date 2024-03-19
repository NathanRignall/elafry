use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc, Mutex};

use command_fds::{CommandFdExt, FdMapping};
use elafry::types::configuration::NonBlockingData;
use uuid::Uuid;

use crate::global_state::Implementation;
use crate::services::communication::Endpoint;
use crate::services::scheduler::{MajorFrame, MinorFrame};

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

struct Background {
    thread: std::thread::JoinHandle<()>,
    sender: Sender<()>,
    actions: Arc<Mutex<Vec<NonBlockingData>>>,
    done_implement: Arc<Mutex<HashMap<Uuid, Implementation>>>,
    done_remove: Arc<Mutex<Vec<Uuid>>>,
}

pub struct ManagementService {
    state: State,
    background: Background,
}

impl ManagementService {
    pub fn new(configuration: String) -> ManagementService {
        let (sender, receiver) = mpsc::channel();
        let non_blocking_actions = Arc::new(Mutex::new(Vec::new()));
        let done_implement = Arc::new(Mutex::new(HashMap::new()));
        let done_remove = Arc::new(Mutex::new(Vec::new()));

        let non_blocking_actions_clone = non_blocking_actions.clone();
        let done_implement_clone = done_implement.clone();
        let done_remove_clone = done_remove.clone();

        let thread = std::thread::spawn(move || {
            loop {
                log::debug!("Waiting for signal");
                match receiver.recv() {
                    Ok(_) => {
                        log::info!("Received signal");

                        // loop through all non-blocking actions
                        let mut non_blocking_actions = non_blocking_actions.lock().unwrap();

                        for action in non_blocking_actions.iter() {
                            match action {
                                NonBlockingData::AddComponent(data) => {
                                    // get the implementation
                                    let implementation = add_component_implementation(
                                        data.component.clone(),
                                        data.core,
                                    );

                                    // add the implementation to the list of done implementations
                                    let mut done_implement = done_implement.lock().unwrap();
                                    done_implement.insert(data.component_id, implementation);
                                }
                                NonBlockingData::RemoveComponent(data) => {
                                    // do something
                                    // add the component to the list of done removes
                                    let mut done_remove = done_remove.lock().unwrap();
                                    done_remove.push(data.component_id);
                                }
                            }
                        }

                        // clear the list of non-blocking actions
                        non_blocking_actions.clear();

                        log::info!("Done processing signal");
                    }
                    Err(_) => {}
                }
            }
        });

        ManagementService {
            state: State::Loading {
                configuration: configuration,
            },
            background: Background {
                thread: thread,
                sender: sender,
                actions: non_blocking_actions_clone,
                done_implement: done_implement_clone,
                done_remove: done_remove_clone,
            },
        }
    }

    pub fn run(&mut self, state: &mut crate::global_state::GlobalState) {
        // get exit message

        let messages = state.get_message(0);

        match messages {
            Some(_) => {
                log::info!("Received kill message");
                state.set_done(true);
            }
            None => {}
        }

        // run the management service state machine
        match &mut self.state {
            State::Idle => {
                // log::debug!("State Idle");

                // check if there are any requests to change the configuration
                let messages = state.get_message(1);

                match messages {
                    Some(message) => {
                        log::debug!("Received message: {:?}", message);

                        // convert message to string
                        let message = std::str::from_utf8(&message.data).unwrap();
                        self.state = State::Loading {
                            configuration: message.to_string(),
                        };
                    }
                    None => {
                        // log::debug!("No message");
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
                                execute_non_blocking(state, status, self.background.sender.clone(), &self.background.actions, &self.background.done_implement, action.data);
                            }

                            // if all non-blocking actions are done, set blocked to false
                            if action_status.values().all(|status| {
                                log::trace!("Waiting for lock on status");
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

fn set_schedule(state: &mut crate::global_state::GlobalState, schedule: Schedule) {
    log::debug!("Setting schedule");

    // check if the schedule is valid by checking if all components in the schedule are in the state and initialized
    for major_frame in &schedule.major_frames {
        for minor_frame in &major_frame.minor_frames {
            match state.get_component(minor_frame.component_id) {
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
    state: &mut crate::global_state::GlobalState,
    data: elafry::types::configuration::BlockingData,
) {
    match data {
        elafry::types::configuration::BlockingData::StartComponent(data) => {
            state.start_component(data.component_id);
        }
        elafry::types::configuration::BlockingData::StopComponent(data) => {
            state.stop_component(data.component_id);
        }
        elafry::types::configuration::BlockingData::AddRoute(data) => {
            state.add_route(
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
            state.remove_route(RouteEndpoint {
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
            });
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

fn add_component_implementation(path: String, core: usize) -> crate::global_state::Implementation {
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
    crate::global_state::Implementation {
        control_socket: crate::global_state::Socket {
            socket: control_socket,
            count: 0,
        },
        data_socket: crate::global_state::Socket {
            socket: data_socket,
            count: 0,
        },
        state_socket: crate::global_state::Socket {
            socket: state_socket,
            count: 0,
        },
        child,
    }
}

fn add_component(
    state: &mut crate::global_state::GlobalState,
    status: &Arc<Mutex<ActionState>>,
    sender: Sender<()>,
    actions: &Arc<Mutex<Vec<NonBlockingData>>>,
    done_implement: &Arc<Mutex<HashMap<Uuid, Implementation>>>,
    id: uuid::Uuid,
    path: String,
    core: usize,
) {
    // get a lock on the status
    log::trace!("Waiting for lock on status");
    let mut status_lock = status.try_lock().unwrap();

    log::debug!("Adding component {} {:?}", id, *status_lock);

    match *status_lock {
        ActionState::Started => {
            // create the component
            state.add_component(id, path, core);

            // set the status to running
            *status_lock = ActionState::Running;
        }
        ActionState::Running => {
            // try get a lock on the actions
            log::trace!("Waiting for lock on actions");
            if let Ok(mut actions) = actions.try_lock() {
                // push the action to the actions vector
                actions.push(NonBlockingData::AddComponent(elafry::types::configuration::AddComponentData {
                    component_id: id,
                    component: path,
                    core: core,
                    version: "0.0.1".to_string(),
                }));

                // send signal to background thread
                sender.send(()).unwrap();

                // set the status to stopped
                *status_lock = ActionState::Stopped;
            } else {
                log::warn!("Failed to get lock on actions");
            }
        }
        ActionState::Stopped => {
            // try get a lock on the done_implement
            log::trace!("Waiting for lock on done_implement");
            if let Ok(mut done_implement) = done_implement.try_lock() {
                // pop the implementation from the done_implement hashmap
                if let Some(implementation) = done_implement.remove(&id) {
                    // put the implementation in the component
                    state.add_component_implementation(id, implementation);

                    // set the status to done
                    *status_lock = ActionState::Completed;
                } else {
                    log::warn!("Component {} not done", id);
                }
            } else {
                log::warn!("Failed to get lock on done_implement");
            }
        }
        ActionState::Completed => {
            log::warn!("Should not be here");
        }
    }

    log::debug!("Created component {}", id);

    log::debug!("Added component {}", id);
}

fn remove_component(
    state: &mut crate::global_state::GlobalState,
    status: &Arc<Mutex<ActionState>>,
    id: uuid::Uuid,
) {
    // get a lock on the status
    log::trace!("Waiting for lock on status");
    let mut status_lock = status.lock().unwrap();

    log::debug!("Removing component {} {:?}", id, *status_lock);

    match *status_lock {
        ActionState::Started => {
            // remove the component
            state.remove_component(id);

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
            // get the component
            match state.get_component_mut(id) {
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
                            state.remove_component_implementation(id);

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
    state: &mut crate::global_state::GlobalState,
    status: &Arc<Mutex<ActionState>>,
    sender: Sender<()>,
    actions: &Arc<Mutex<Vec<NonBlockingData>>>,
    done_implement: &Arc<Mutex<HashMap<Uuid, Implementation>>>,
    data: elafry::types::configuration::NonBlockingData,
) {
    match data {
        elafry::types::configuration::NonBlockingData::AddComponent(data) => {
            add_component(state, status, sender, actions, done_implement, data.component_id, data.component, data.core);
        }
        elafry::types::configuration::NonBlockingData::RemoveComponent(data) => {
            remove_component(state, status, data.component_id);
        }
    }
}
