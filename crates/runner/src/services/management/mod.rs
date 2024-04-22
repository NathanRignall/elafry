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
    Waiting {
        configuration: String,
    },
    Loading,
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
    done_configuration: Arc<Mutex<Option<elafry::types::configuration::Configuration>>>,
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
        let done_configuration = Arc::new(Mutex::new(None));
        let done_implement = Arc::new(Mutex::new(HashMap::new()));
        let done_remove = Arc::new(Mutex::new(Vec::new()));

        let non_blocking_actions_clone = non_blocking_actions.clone();
        let done_configuration_clone = done_configuration.clone();
        let done_implement_clone = done_implement.clone();
        let done_remove_clone = done_remove.clone();

        let thread = std::thread::spawn(move || {
            // set core affinity to core 0
            unsafe {
                let mut cpu_set: libc::cpu_set_t = std::mem::zeroed();
                libc::CPU_SET(0, &mut cpu_set);
                let ret = libc::sched_setaffinity(0, std::mem::size_of_val(&cpu_set), &cpu_set);
                if ret != 0 {
                    println!("Failed to set affinity");
                }
            }

            // use libc to set the process sechdeuler to SCHEDULER NORMAL with a priority of -20
            unsafe {
                let ret = libc::pthread_setschedparam(
                    libc::pthread_self(),
                    libc::SCHED_IDLE,
                    &libc::sched_param { sched_priority: 0 },
                );
                if ret != 0 {
                    println!("Failed to set scheduler");
                }
            }

            background::main(
                receiver,
                non_blocking_actions,
                done_configuration,
                done_implement,
                done_remove,
            );
        });

        ManagementService {
            state: State::Waiting { configuration },
            background: Background {
                _thread: thread,
                sender,
                data: BackgroundData {
                    actions: non_blocking_actions_clone,
                    done_configuration: done_configuration_clone,
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

        // log::debug!("Running management service");

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
                        self.state = State::Waiting {
                            configuration: message.to_string(),
                        };
                    }
                    None => {
                        // log::debug!("No message");
                    }
                }
            }
            State::Waiting { configuration } => {
                log::debug!("State Waiting");

                // try get a lock on actions
                if let Ok(mut actions) = self.background.data.actions.try_lock() {
                    // push the action to the actions vector
                    actions.push(
                        background::NonBlockingImplementationData::LoadConfiguration(
                            background::LoadConfiguration {
                                path: configuration.to_string(),
                            },
                        ),
                    );

                    // send signal to background thread
                    self.background.sender.send(()).unwrap();

                    // change state to loading
                    self.state = State::Loading;
                } else {
                    log::warn!("Failed to get lock on actions");
                }
            }

            State::Loading => {
                log::debug!("State Loading");

                // try get a lock on done_configuration
                if let Ok(mut done_configuration) =
                    self.background.data.done_configuration.try_lock()
                {
                    // get clone of done_configuration
                    let configuration = done_configuration.clone();

                    match configuration {
                        Some(configuration) => {
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

                                // empty the done_configuration
                                *done_configuration = None;

                                // return
                                return;
                            }
                        }
                        None => {
                            log::debug!("Not received configuration from background thread");
                        }
                    }
                } else {
                    log::warn!("Failed to get lock on done_configuration");
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
                    },
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
