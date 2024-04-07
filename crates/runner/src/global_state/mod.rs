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
    pub data_socket: Socket,
    pub state_socket: Socket,
    pub child: std::process::Child,
    pub child_pid: libc::pid_t,
}

pub struct Socket {
    pub socket: UnixStream,
    pub count: u8,
}

pub struct StateSync {
    pub source: StateEndpoint,
    pub target: StateEndpoint,
    pub status: StateSyncStatus,
}

pub struct StateEndpoint {
    pub component_id: uuid::Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StateSyncStatus {
    Created,
    Started,
    Synced,
}

pub struct GlobalState {
    pub components: HashMap<uuid::Uuid, Component>,
    pub routes: HashMap<RouteEndpoint, RouteEndpoint>,
    pub schedule: Schedule,
    pub messages: HashMap<u32, Vec<Message>>,
    pub state_sync: HashMap<uuid::Uuid, StateSync>,
    done: bool,
}

impl GlobalState {
    pub fn new() -> Self {
        GlobalState {
            components: HashMap::new(),
            routes: HashMap::new(),
            schedule: Schedule {
                period: std::time::Duration::from_micros(1000),
                major_frames: vec![],
            },
            messages: HashMap::new(),
            state_sync: HashMap::new(),
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
                    panic!("Component {} not initialized", id);
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
        log::info!("Adding component {}", id);

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
    }

    pub fn get_component(&self, id: uuid::Uuid) -> Option<&Component> {
        self.components.get(&id)
    }

    pub fn get_component_mut(&mut self, id: uuid::Uuid) -> Option<&mut Component> {
        self.components.get_mut(&id)
    }

    pub fn total_components(&self) -> usize {
        self.components.len()
    }

    pub fn set_schedule(&mut self, schedule: Schedule) {
        log::info!("Setting schedule");

        // check if the schedule is valid by checking if all components in the schedule are in the state and initialized
        for major_frame in &schedule.major_frames {
            for minor_frame in &major_frame.minor_frames {
                match self.get_component(minor_frame.component_id) {
                    Some(component) => {
                        if component.implentation.is_none() {
                            panic!("Component {} not initialized", minor_frame.component_id);
                        }
                    }
                    None => {
                        panic!("Component {} not found", minor_frame.component_id);
                    }
                }
            }
        }

        // set the schedule in the state
        self.schedule = schedule;

        // print the schedule duration
        log::debug!("Schedule duration: {:?}", self.schedule.period);
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

    pub fn add_state_sync(&mut self, state_sync_id: uuid::Uuid, source: StateEndpoint, target: StateEndpoint, status: StateSyncStatus) {
        log::debug!("Adding state sync {}", state_sync_id);

        // add the state sync to the state
        self.state_sync.insert(
            state_sync_id,
            StateSync {
                source,
                target,
                status,
            },
        );
    }

    pub fn remove_state_sync(&mut self, state_sync_id: uuid::Uuid) {
        log::debug!("Removing state sync {}", state_sync_id);

        // remove the state sync from the state
        self.state_sync.remove(&state_sync_id);
    }

    pub fn get_state_sync_status(&self, state_sync_id: uuid::Uuid) -> StateSyncStatus {
        log::debug!("Getting state sync {} status", state_sync_id);

        // check if state_sync_id exists in hashmap
        if self.state_sync.contains_key(&state_sync_id) {
            // get status from state_sync
            let state_sync = self.state_sync.get(&state_sync_id).unwrap();
            return state_sync.status;
        } else {
            panic!("State sync {} not found", state_sync_id);
        }
    }

    pub fn set_state_sync_status(&mut self, state_sync_id: uuid::Uuid, status: StateSyncStatus) {
        log::debug!("Setting state sync {} status to {:?}", state_sync_id, status);

        // check if state_sync_id exists in hashmap
        if self.state_sync.contains_key(&state_sync_id) {
            // set status in state_sync
            let state_sync = self.state_sync.get_mut(&state_sync_id).unwrap();
            state_sync.status = status;
        } else {
            panic!("State sync {} not found", state_sync_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::services::{communication::Endpoint, scheduler::{MajorFrame, MinorFrame}};

    use super::*;

    #[test] 
    fn test_global_state_component() {
        let mut state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        let path = "path".to_string();
        let core = 0;
        let implementation = Implementation {
            data_socket: Socket {
                socket: UnixStream::pair().unwrap().0,
                count: 0,
            },
            state_socket: Socket {
                socket: UnixStream::pair().unwrap().0,
                count: 0,
            },
            child: std::process::Command::new("ls").spawn().unwrap(),
            child_pid: 0,
        };

        state.add_component(id, path.clone(), core);
        assert_eq!(state.total_components(), 1);
        assert_eq!(state.get_component(id).unwrap().path, path);
        assert_eq!(state.get_component(id).unwrap().core, core);
        assert_eq!(state.get_component(id).unwrap().implentation.is_none(), true); 

        state.get_component_mut(id).unwrap().times.push(1);
        assert_eq!(state.get_component(id).unwrap().times.len(), 1);

        state.add_component_implementation(id, implementation);
        assert_eq!(state.get_component(id).unwrap().implentation.is_some(), true);

        state.start_component(id);
        assert_eq!(state.get_component(id).unwrap().run, true);

        state.stop_component(id);
        assert_eq!(state.get_component(id).unwrap().run, false);

        state.remove_component_implementation(id);
        assert_eq!(state.get_component(id).unwrap().implentation.is_none(), true);

        state.remove_component(id);
        assert_eq!(state.total_components(), 1);
        assert_eq!(state.get_component(id).unwrap().remove, true);
        assert_eq!(state.get_component(id).unwrap().run, false);
        assert_eq!(state.get_component(id).unwrap().implentation.is_none(), true);
    }

    #[test]
    #[should_panic]
    fn test_global_state_component_not_found() {
        let state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        state.get_component(id).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_global_state_component_not_found_mut() {
        let mut state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        state.get_component_mut(id).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_global_state_component_not_found_start() {
        let mut state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        state.start_component(id);
    }

    #[test]
    #[should_panic]
    fn test_global_state_component_not_found_stop() {
        let mut state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        state.stop_component(id);
    }

    #[test]
    #[should_panic]
    fn test_global_state_component_not_found_add_implementation() {
        let mut state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        let implementation = Implementation {
            data_socket: Socket {
                socket: UnixStream::pair().unwrap().0,
                count: 0,
            },
            state_socket: Socket {
                socket: UnixStream::pair().unwrap().0,
                count: 0,
            },
            child: std::process::Command::new("ls").spawn().unwrap(),
            child_pid: 0,
        };

        state.add_component_implementation(id, implementation);
    }

    #[test]
    #[should_panic]
    fn test_global_state_component_not_found_remove() {
        let mut state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        state.remove_component(id);
    }

    #[test]
    #[should_panic]
    fn test_global_state_component_not_found_implementation() {
        let mut state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        state.remove_component_implementation(id);
    }

    #[test]
    #[should_panic]
    fn test_global_state_component_not_initialized() {
        let mut state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        let path = "path".to_string();
        let core = 0;

        state.add_component(id, path.clone(), core);
        state.start_component(id);
    }

    #[test]
    fn test_global_state_schedule_empty() {
        let mut state = GlobalState::new();

        let schedule = Schedule {
            period: std::time::Duration::from_secs(1),
            major_frames: vec![],
        };

        state.set_schedule(schedule);

        assert_eq!(state.schedule.period, std::time::Duration::from_secs(1));
    }

    #[test]
    fn test_global_state_schedule() {
        let mut state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        let path = "path".to_string();
        let core = 0;
        let implementation = Implementation {
            data_socket: Socket {
                socket: UnixStream::pair().unwrap().0,
                count: 0,
            },
            state_socket: Socket {
                socket: UnixStream::pair().unwrap().0,
                count: 0,
            },
            child: std::process::Command::new("ls").spawn().unwrap(),
            child_pid: 0,
        };

        state.add_component(id, path.clone(), core);
        state.add_component_implementation(id, implementation);

        let schedule = Schedule {
            period: std::time::Duration::from_secs(1),
            major_frames: vec![MajorFrame {
                minor_frames: vec![MinorFrame {
                    component_id: id,
                    deadline: std::time::Duration::from_secs(1),
                }],
            }],
        };

        state.set_schedule(schedule);

        assert_eq!(state.schedule.period, std::time::Duration::from_secs(1));
    }

    #[test]
    #[should_panic]
    fn test_global_state_schedule_invalid() {
        let mut state = GlobalState::new();

        let schedule = Schedule {
            period: std::time::Duration::from_secs(1),
            major_frames: vec![MajorFrame {
                minor_frames: vec![MinorFrame {
                    component_id: uuid::Uuid::new_v4(),
                    deadline: std::time::Duration::from_secs(1),
                }],
            }],
        };

        state.set_schedule(schedule);
    }

    #[test]
    #[should_panic]
    fn test_global_state_schedule_invalid_component() {
        let mut state = GlobalState::new();

        let id = uuid::Uuid::new_v4();
        let path = "path".to_string();
        let core = 0;

        state.add_component(id, path.clone(), core);

        let schedule = Schedule {
            period: std::time::Duration::from_secs(1),
            major_frames: vec![MajorFrame {
                minor_frames: vec![MinorFrame {
                    component_id: id,
                    deadline: std::time::Duration::from_secs(1),
                }],
            }],
        };

        state.set_schedule(schedule);
    }

    #[test]
    fn test_global_state_route() {
        let mut state = GlobalState::new();

        let source = RouteEndpoint {
            endpoint: Endpoint::Component(uuid::Uuid::new_v4()),
            channel_id: 0,
        };

        let target = RouteEndpoint {
            endpoint: Endpoint::Component(uuid::Uuid::new_v4()),
            channel_id: 1,
        };

        state.add_route(source, target);

        assert_eq!(state.routes.len(), 1);
        assert_eq!(state.routes.get(&source).unwrap().endpoint, target.endpoint);

        state.remove_route(source);

        assert_eq!(state.routes.len(), 0);
    }

    #[test]
    fn test_global_state_message() {
        let mut state = GlobalState::new();

        let channel_id = 0;
        let message = Message {
            channel_id,
            data: vec![],
            count: 0,
        };

        state.messages.insert(channel_id, vec![message.clone()]);

        let message = state.get_message(channel_id).unwrap();
        assert_eq!(message.channel_id, channel_id);
        assert_eq!(state.messages.get(&channel_id).unwrap().len(), 0);
    }

    #[test]
    fn test_global_state_message_empty() {
        let mut state = GlobalState::new();

        let channel_id = 0;

        let message = state.get_message(channel_id);
        assert_eq!(message.is_none(), true);
    }

    #[test]
    fn test_global_state_done() {
        let mut state = GlobalState::new();

        assert_eq!(state.get_done(), false);

        state.set_done(true);
        assert_eq!(state.get_done(), true);
    }
    
}