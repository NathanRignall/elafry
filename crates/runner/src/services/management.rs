use crate::services::communication::Endpoint;
use crate::services::scheduler::{MajorFrame, MinorFrame};
use crate::Component;

use super::communication::RouteEndpoint;
use super::scheduler::Schedule;

pub struct ManagementService {
    configuration: elafry::types::configuration::Configuration,
}

impl ManagementService {
    pub fn new() -> Self {
        ManagementService {
            configuration: elafry::types::configuration::Configuration { tasks: Vec::new() },
        }
    }

    fn add_component(
        &mut self,
        state: &mut crate::GlobalState,
        id: uuid::Uuid,
        path: &str,
        core: usize,
    ) {
        log::debug!("Adding component {}", path);

        // create the component
        let component = Component {
            run: false,
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

    fn start_component(&mut self, state: &mut crate::GlobalState, id: uuid::Uuid) {
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

    fn stop_component(&mut self, state: &mut crate::GlobalState, id: uuid::Uuid) {
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

    fn remove_component(&mut self, state: &crate::GlobalState, id: uuid::Uuid) {}

    fn add_route(
        &mut self,
        state: &mut crate::GlobalState,
        source: RouteEndpoint,
        target: RouteEndpoint,
    ) {
        log::debug!("Adding route from {:?} to {:?}", source, target);

        // add the route to the state
        state.routes.insert(source, target);

        log::debug!("Added route");
    }

    fn remove_route(&mut self, state: &mut crate::GlobalState, from: RouteEndpoint) {
        log::debug!("Removing route from {:?}", from);

        // remove the route from the state
        state.routes.remove(&from);

        log::debug!("Removed route from {:?}", from);
    }

    fn set_schedule(&mut self, state: &mut crate::GlobalState, schedule: Schedule) {
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

    pub fn execute(
        &mut self,
        state: &mut crate::GlobalState,
        action: elafry::types::configuration::Action,
    ) {
        log::debug!("Agent::execute");
        match action {
            elafry::types::configuration::Action::AddComponent(action) => {
                self.add_component(
                    state,
                    action.data.component_id,
                    &action.data.component,
                    action.data.core,
                );
            }
            elafry::types::configuration::Action::StartComponent(action) => {
                self.start_component(state, action.data.component_id);
            }
            elafry::types::configuration::Action::StopComponent(action) => {
                self.stop_component(state, action.data.component_id);
            }
            elafry::types::configuration::Action::RemoveComponent(action) => {
                self.remove_component(state, action.data.component_id);
            }
            elafry::types::configuration::Action::AddRoute(action) => {
                self.add_route(
                    state,
                    RouteEndpoint {
                        endpoint: match &action.data.source.endpoint {
                            elafry::types::configuration::Endpoint::Component(endpoint) => {
                                Endpoint::Component(endpoint.clone())
                            }
                            elafry::types::configuration::Endpoint::Address(endpoint) => {
                                Endpoint::Address(endpoint.parse().unwrap())
                            }
                            elafry::types::configuration::Endpoint::Runner => Endpoint::Runner,
                        },
                        channel_id: action.data.source.channel_id,
                    },
                    RouteEndpoint {
                        endpoint: match &action.data.target.endpoint {
                            elafry::types::configuration::Endpoint::Component(endpoint) => {
                                Endpoint::Component(endpoint.clone())
                            }
                            elafry::types::configuration::Endpoint::Address(endpoint) => {
                                Endpoint::Address(endpoint.clone().parse().unwrap())
                            }
                            elafry::types::configuration::Endpoint::Runner => Endpoint::Runner,
                        },
                        channel_id: action.data.target.channel_id,
                    },
                );
            }
            elafry::types::configuration::Action::RemoveRoute(action) => {
                self.remove_route(
                    state,
                    RouteEndpoint {
                        endpoint: match &action.data.source.endpoint {
                            elafry::types::configuration::Endpoint::Component(endpoint) => {
                                Endpoint::Component(endpoint.clone())
                            }
                            elafry::types::configuration::Endpoint::Address(endpoint) => {
                                Endpoint::Address(endpoint.parse().unwrap())
                            }
                            elafry::types::configuration::Endpoint::Runner => Endpoint::Runner,
                        },
                        channel_id: action.data.source.channel_id,
                    },
                );
            }
            elafry::types::configuration::Action::SetSchedule(action) => {
                self.set_schedule(
                    state,
                    Schedule {
                        period: std::time::Duration::from_micros(1_000_000 / action.data.frequency),
                        major_frames: action
                            .data
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
        log::debug!("Agent::execute done");
    }

    pub fn load(&mut self, state: &mut crate::GlobalState, configuration: String) {
        log::debug!("Agent::load");

        // open the configuration file
        let file = std::fs::File::open(configuration).unwrap();

        // read the configuration file
        let configuration: Result<elafry::types::configuration::Configuration, serde_yaml::Error> =
            serde_yaml::from_reader(file);

        // check if the configuration file was read successfully
        match configuration {
            Ok(configuration) => {
                log::debug!("Configuration file read successfully");
                self.configuration = configuration;
            }
            Err(error) => {
                log::error!("Error reading configuration file: {}", error);
            }
        }

        log::debug!("Agent::load done");
    }

    pub fn run(&mut self, state: &mut crate::GlobalState) {
        let mut new_configuration = Option::None;

        // get messages
        if state.messages.contains_key(&1) {
            let messages = state.messages.get_mut(&1).unwrap();
                
            match messages.pop() {
                Some(message) => {
                    log::debug!("Received message: {:?}", message);

                    // convert message to string
                    let message = std::str::from_utf8(&message.data).unwrap();

                    new_configuration = Some(message.to_string());
                }
                None => {}
            }
        }

        // check if there is a new configuration
        if let Some(new_configuration) = new_configuration {
            log::info!("New configuration: {}", new_configuration);

            // load new configuration
            self.load(state, new_configuration);
        }

        // check if there are any tasks in the configuration
        if self.configuration.tasks.is_empty() {
            return;
        }

        // get actions from first task
        let actions = self.configuration.tasks[0].actions.clone();

        log::info!("Actions: {:?}", self.configuration.tasks[0].id);

        // execute actions
        for action in actions {
            self.execute(state, action);
        }

        // remove first task
        self.configuration.tasks.remove(0);

        log::debug!("Ran management service");
    }
}
