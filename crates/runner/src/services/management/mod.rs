use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc, Mutex};

use uuid::Uuid;

use crate::global_state::{Implementation, StateEndpoint};
use crate::services::communication::Endpoint;
use crate::services::scheduler::{MajorFrame, MinorFrame};

use super::communication::RouteEndpoint;
use super::scheduler::Schedule;

pub mod background;

enum State {
    Idle,
    Loading {
        configuration: String,
    },
    Running {
        current_task: usize,
        tasks: Vec<elafry::types::configuration::Task>,
        blocked: bool,
        action_status: HashMap<uuid::Uuid, ActionState>,
    },
}

#[derive(PartialEq, Debug)]
pub enum ActionState {
    Started,
    Running,
    Stopped,
    Completed,
}

struct Background {
    _thread: std::thread::JoinHandle<()>,
    sender: Sender<()>,
    data: BackgroundData,
}

#[derive(Clone)]
struct BackgroundData {
    actions: Arc<Mutex<Vec<background::NonBlockingImplementationData>>>,
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
            // use libc to set the process sechdeuler to SCHEDULER NORMAL with a priority of -20
            unsafe {
                let ret = libc::pthread_setschedparam(
                    libc::pthread_self(),
                    libc::SCHED_OTHER,
                    &libc::sched_param { sched_priority: 0 },
                );
                if ret != 0 {
                    println!("Failed to set scheduler");
                }
            }

            background::main(receiver, non_blocking_actions, done_implement, done_remove);
        });

        ManagementService {
            state: State::Loading { configuration },
            background: Background {
                _thread: thread,
                sender,
                data: BackgroundData {
                    actions: non_blocking_actions_clone,
                    done_implement: done_implement_clone,
                    done_remove: done_remove_clone,
                },
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
                let path = format!("configuration/{}", configuration);
                let file = std::fs::File::open(path).unwrap();
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
                                Self::execute_blocking(state, action.data);
                            }
                        }
                    }
                    elafry::types::configuration::Action::NonBlocking(actions) => {
                        log::debug!("Non-blocking actions");
                        if *blocked {
                            for action in actions {
                                let status = action_status.get_mut(&action.id).unwrap();
                                Self::execute_non_blocking(
                                    state,
                                    status,
                                    self.background.sender.clone(),
                                    self.background.data.clone(),
                                    action.data,
                                );
                            }

                            // if all non-blocking actions are done, set blocked to false
                            if action_status
                                .values()
                                .all(|status| *status == ActionState::Completed)
                            {
                                log::debug!("All non-blocking actions done");
                                *blocked = false;
                            }
                        } else {
                            for action in actions {
                                action_status.insert(action.id, ActionState::Started);
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
                            action_status: HashMap::new(),
                        };
                    } else {
                        self.state = State::Idle;
                    }
                }
            }
        }
    }

    fn execute_blocking(
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
                state.set_schedule(Schedule {
                    period: std::time::Duration::from_micros(data.deadline),
                    major_frames: data
                        .major_frames
                        .into_iter()
                        .map(|frame| MajorFrame {
                            minor_frames: frame
                                .minor_frames
                                .into_iter()
                                .map(|frame| MinorFrame {
                                    component_id: frame.component_id,
                                    deadline: std::time::Duration::from_micros(frame.deadline),
                                })
                                .collect(),
                        })
                        .collect(),
                });
            }
            elafry::types::configuration::BlockingData::AddStateSync(data) => {
                state.add_state_sync(
                    data.state_sync_id,
                    StateEndpoint {
                        component_id: data.source.component_id,
                    },
                    StateEndpoint {
                        component_id: data.target.component_id,
                    }
                );
            }
            elafry::types::configuration::BlockingData::RemoveStateSync(data) => {
                state.remove_state_sync(data.state_sync_id);
            }
        }
    }

    fn execute_non_blocking(
        state: &mut crate::global_state::GlobalState,
        status: &mut ActionState,
        sender: Sender<()>,
        background: BackgroundData,
        data: elafry::types::configuration::NonBlockingData,
    ) {
        // dereference background data
        let (actions, done_implement, done_remove) = (
            background.actions.clone(),
            background.done_implement.clone(),
            background.done_remove.clone(),
        );

        match data {
            elafry::types::configuration::NonBlockingData::AddComponent(data) => {
                background::add_component(state, status, sender, actions, done_implement, data);
            }
            elafry::types::configuration::NonBlockingData::RemoveComponent(data) => {
                background::remove_component(state, status, sender, actions, done_remove, data);
            }
            elafry::types::configuration::NonBlockingData::WaitStateSync(data) => {
                background::wait_state_sync(state, status, data);
            }
        }
    }
}
