use std::sync::{Arc, Mutex};

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
        current_action: Arc<Mutex<Option<uuid::Uuid>>>,
    },
}

pub struct ManagementService {
    state: State,
}

impl ManagementService {
    pub fn new(configuration: String) -> ManagementService {
        ManagementService {
            state: State::Loading { configuration: configuration },
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
        match &self.state {
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
                log::debug!("Loading");

                // read the configuration file
                let file = std::fs::File::open(configuration).unwrap();
                let configuration: Result<elafry::types::configuration::Configuration, serde_yaml::Error> =
                    serde_yaml::from_reader(file);

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
                                current_action: Arc::new(Mutex::new(None)),
                            };
                        }
                    }
                    Err(error) => {
                        log::error!("Error reading configuration file: {}", error);
                        self.state = State::Idle;
                    }
                }
            }
            State::Running { current_task, tasks, current_action } => {
                log::debug!("Running");

                // get actions from first task
                let actions = tasks[*current_task].actions.clone();

                // execute actions
                match actions {
                    elafry::types::configuration::Action::Blocking(actions) => {
                        for action in actions {
                            if current_action.lock().unwrap().is_some() {
                                log::warn!("Action already running");
                            } else {
                                current_action.lock().unwrap().replace(action.id);
                                execute_blocking(state, action.data);
                                current_action.lock().unwrap().take();
                            }
                            
                        }
                    }
                    elafry::types::configuration::Action::NonBlocking(actions) => {
                        for action in actions {
                            if current_action.lock().unwrap().is_some() {
                                log::warn!("Action already running");
                            } else {
                                current_action.lock().unwrap().replace(action.id);
                                execute_non_blocking(current_action.clone(), state, action.data);
                                current_action.lock().unwrap().take();
                            }
                        }
                    }
                }

                // check if all actions are done and move to next task
                if current_action.lock().unwrap().is_none() {
                    // check if there are more tasks
                    if *current_task < tasks.len() - 1 {
                        self.state = State::Running {
                            current_task: *current_task + 1,
                            tasks: tasks.clone(),
                            current_action: current_action.clone(),
                        };
                    } else {
                        self.state = State::Idle;
                    }
                }
            }
        }
    }
}

fn add_component(
    state: &mut crate::GlobalState,
    id: uuid::Uuid,
    path: &str,
    core: usize,
) {
    log::debug!("Adding component {}", path);

    // create the component
    let component = Component {
        run: false,
        remove: false,
        path: path.to_string(),
        core,
        implentation: None,
        times: Vec::new(),
    };

    log::debug!("Created component {}", id);

    // add the component to the state
    state.components.insert(id, component);

    log::debug!("Added component {}", id);
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

fn remove_component(state: &mut crate::GlobalState, id: uuid::Uuid) {
    log::debug!("Removing component {}", id);

    // get the component
    match state.components.get_mut(&id) {
        Some(component) => {
            log::debug!("Got component {}", id);

            // flag the component for removal
            component.remove = true;
            component.run = false;

            log::debug!("Removed component {}", id);
        }
        None => {
            panic!("Component {} not found", id)
        }
    }
}

fn add_route(
    state: &mut crate::GlobalState,
    source: RouteEndpoint,
    target: RouteEndpoint,
) {
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

pub fn execute_non_blocking(
    current_action: Arc<Mutex<Option<uuid::Uuid>>>,
    state: &mut crate::GlobalState,
    data: elafry::types::configuration::NonBlockingData,
) {
    match data {
        elafry::types::configuration::NonBlockingData::AddComponent(data) => {
            add_component(state, data.component_id, &data.component, data.core);
            current_action.lock().unwrap().take();
        }
        elafry::types::configuration::NonBlockingData::RemoveComponent(data) => {
            remove_component(state, data.component_id);
        }
    }
}