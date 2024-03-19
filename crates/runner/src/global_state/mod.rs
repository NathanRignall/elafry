use std::{collections::HashMap, os::unix::net::UnixStream};

use crate::services::{communication::RouteEndpoint, scheduler::Schedule};
use elafry::types::communication::Message;

pub struct Component {
    pub run: bool,
    pub remove: bool,
    pub path: String,
    pub core: usize,
    pub implentation: Option<Implementation>,
    pub times: Vec<u64>,
}

pub struct Implementation {
    pub control_socket: Socket,
    pub data_socket: Socket,
    pub state_socket: Socket,
    pub child: std::process::Child,
}

pub struct Socket {
    pub socket: UnixStream,
    pub count: u8,
}

pub struct GlobalState {
    pub components: HashMap<uuid::Uuid, Component>,
    pub routes: HashMap<RouteEndpoint, RouteEndpoint>,
    pub schedule: Schedule,
    pub messages: HashMap<u32, Vec<Message>>,
    done: bool,
}

impl GlobalState {
    pub fn new() -> Self {
        GlobalState {
            components: HashMap::new(),
            routes: HashMap::new(),
            schedule: Schedule {
                period: std::time::Duration::from_secs(1),
                major_frames: vec![],
            },
            messages: HashMap::new(),
            done: false,
        }
    }

    pub fn get_done(&self) -> bool {
        self.done
    }

    pub fn set_done(&mut self, done: bool) {
        self.done = done;
    }

    pub fn start_component(&mut self, id: uuid::Uuid) {
        log::debug!("Starting component {}", id);

        // get the component
        match self.components.get_mut(&id) {
            Some(component) => {
                // don't start if not finish initializing
                if component.implentation.is_none() {
                    log::error!("Component {} not initialized", id);
                    return;
                }

                // start the component
                component.run = true;
            }
            None => {
                panic!("Component {} not found", id)
            }
        }
    }

    pub fn stop_component(&mut self, id: uuid::Uuid) {
        log::debug!("Stopping component {}", id);

        // get the component
        match self.components.get_mut(&id) {
            Some(component) => {
                // stop the component
                component.run = false;
            }
            None => {
                panic!("Component {} not found", id)
            }
        }
    }

    pub fn add_route(&mut self, source: RouteEndpoint, target: RouteEndpoint) {
        log::debug!("Adding route from {:?} to {:?}", source, target);

        // add the route to the state
        self.routes.insert(source, target);
    }

    pub fn remove_route(&mut self, from: RouteEndpoint) {
        log::debug!("Removing route from {:?}", from);

        // remove the route from the state
        self.routes.remove(&from);
    }

    pub fn add_component(&mut self, id: uuid::Uuid, path: String, core: usize) {
        log::debug!("Adding component {}", id);

        // add the component to the state
        self.components.insert(
            id,
            Component {
                run: false,
                remove: false,
                path,
                core,
                implentation: None,
                times: vec![],
            },
        );
    }

    pub fn add_component_implementation(&mut self, id: uuid::Uuid, implementation: Implementation) {
        log::debug!("Adding implementation to component {}", id);

        // get the component
        match self.components.get_mut(&id) {
            Some(component) => {
                // add the implementation
                component.implentation = Some(implementation);
            }
            None => {
                panic!("Component {} not found", id)
            }
        }
    }

    pub fn remove_component(&mut self, id: uuid::Uuid) {
        log::debug!("Removing component {}", id);

        // get the component
        match self.components.get_mut(&id) {
            Some(component) => {
                // remove the component
                component.remove = true;
                component.run = false;
            }
            None => {
                panic!("Component {} not found", id)
            }
        }
    }

    pub fn remove_component_implementation(&mut self, id: uuid::Uuid) {
        log::debug!("Removing implementation from component {}", id);

        // get the component
        match self.components.get_mut(&id) {
            Some(component) => {
                // remove the implementation
                component.implentation = None;
            }
            None => {
                panic!("Component {} not found", id)
            }
        }

        // remove the component
        self.components.remove(&id);
    }

    pub fn get_component(&self, id: uuid::Uuid) -> Option<&Component> {
        self.components.get(&id)
    }

    pub fn get_component_mut(&mut self, id: uuid::Uuid) -> Option<&mut Component> {
        self.components.get_mut(&id)
    }

    // pub fn get_component_iter(&self) -> impl Iterator<Item = (&uuid::Uuid, &Component)> {
    //     self.components.iter()
    // }

    // pub fn get_component_iter_mut(&mut self) -> impl Iterator<Item = (&uuid::Uuid, &mut Component)> {
    //     self.components.iter_mut()
    // }

    pub fn total_components(&self) -> usize {
        self.components.len()
    }

    // pub fn get_route(&self, source: RouteEndpoint) -> Option<&RouteEndpoint> {
    //     self.routes.get(&source)
    // }

    // pub fn get_route_destination(&self, source: RouteEndpoint) -> Option<RouteEndpoint> {
    //     self.routes.get(&source).cloned()
    // }

    // pub fn get_route_mut(&mut self, source: RouteEndpoint) -> Option<&mut RouteEndpoint> {
    //     self.routes.get_mut(&source)
    // }

    // pub fn get_route_iter(&self) -> impl Iterator<Item = (&RouteEndpoint, &RouteEndpoint)> {
    //     self.routes.iter()
    // }

    // pub fn get_route_iter_mut(&mut self) -> impl Iterator<Item = (&RouteEndpoint, &mut RouteEndpoint)> {
    //     self.routes.iter_mut()
    // }

    // pub fn total_routes(&self) -> usize {
    //     self.routes.len()
    // }

    // pub fn get_component_in_minor_frame(&mut self, index: usize) -> impl Iterator<Item = &mut Component> {
    //     let mut components = vec![];
    //     for frame in &self.schedule.major_frames[index].minor_frames {
    //         if let Some(component) = self.components.get_mut(&frame.component_id) {
    //             components.push(component);
    //         }
    //     }

    //     components.into_iter()
    // }

    pub fn set_schedule(&mut self, schedule: Schedule) {
        log::debug!("Setting schedule");

        // check if the schedule is valid by checking if all components in the schedule are in the state and initialized
        for major_frame in &schedule.major_frames {
            for minor_frame in &major_frame.minor_frames {
                match self.get_component(minor_frame.component_id) {
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
        self.schedule = schedule;
    }

    pub fn get_message(&mut self, channel_id: u32) -> Option<Message> {
        // check if channel_id exists in hashmap
        if self.messages.contains_key(&channel_id) {
            // get message from vector
            let messages = self.messages.get_mut(&channel_id).unwrap();
            let message = messages.pop();
            return message;
        } else {
            return None;
        }
    }

    // pub fn add_message(&mut self, message: Message) {
    //     let channel_id = message.channel_id;
    //     let messages = self.messages.entry(channel_id).or_insert(vec![]);
    //     messages.push(message);
    // }
}
