use std::{collections::HashMap, io::{Read, Write}};

pub struct StateService {
    input_state: HashMap<uuid::Uuid, Vec<u8>>,
}

impl StateService {
    pub fn new() -> Self {
        StateService {
            input_state: HashMap::new(),
        }
    }

    pub fn run(&mut self, state: &mut crate::global_state::GlobalState) {
        // clear input_state and output_state
        self.input_state.clear();

        // check for data on components
        for (id, component) in state.components.iter_mut() {
            let mut length_buf = [0; 4];

            // loop for a maximum of 10 times until no more data is available
            for _ in 0..100 {
                match &mut component.implentation {
                    Some(implentation) => {
                        match implentation.state_socket.socket.read_exact(&mut length_buf) {
                            Ok(_) => {
                                // get length of message
                                let length = u32::from_be_bytes(length_buf);

                                // don't read if length is 0
                                if length == 0 {
                                    log::error!("Length is 0");
                                    continue;
                                }

                                // if length is greater than 1000, skip
                                if length > 1000 {
                                    log::error!("Length is greater than 1000");
                                    continue;
                                }

                                // create buffer with length
                                let state_buf = {
                                    let mut buf = vec![0; length as usize];
                                    match implentation.state_socket.socket.read_exact(&mut buf) {
                                        Ok(_) => buf,
                                        Err(e) => {
                                            log::error!(
                                                "Failed to read from socket; err = {:?}",
                                                e
                                            );
                                            continue;
                                        }
                                    }
                                };

                                // set input_state
                                self.input_state.insert(*id, state_buf);
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                break;
                            }
                            Err(e) => {
                                log::error!("Failed to read from socket; err = {:?}", e);
                                break;
                            }
                        }
                    }
                    None => {
                        continue;
                    }
                }
            }
        }

        // for each item in state_sync hashmap, do something
        for (id, state_sync) in state.state_sync.iter_mut() {
            // check if requested to sync
            if state_sync.status == crate::global_state::StateSyncStatus::Created {
                continue;
            }

            // get state from component_state hashmap
            let input_state = match self.input_state.get(&state_sync.source.component_id) {
                Some(state) => state,
                None => {
                    log::debug!("No state for component_id = {:?}", state_sync.source.component_id);
                    continue;
                }
            };

            // copy state to output_state
            let output_state = input_state;

            // get the component implementation
            let component = match state.components.get_mut(&state_sync.target.component_id) {
                Some(component) => component,
                None => {
                    log::error!("Failed to get component for component_id = {:?}", id);
                    continue;
                }
            };

            // write the state to the component
            let length = output_state.len() as u32;
            let mut length_buf = length.to_be_bytes().to_vec();
            length_buf.append(&mut output_state.clone());
            // println!("Sending message: {:?}", length_buf);

            // if going to block, don't send message
            match component.implentation.as_mut() {
                Some(implentation) => {
                    match implentation.state_socket.socket.write_all(&length_buf) {
                        Ok(_) => {
                            // set sync status to synced
                            log::trace!("Wrote state to component_id = {:?}", id);
                            state_sync.status = crate::global_state::StateSyncStatus::Synced;
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            log::error!("Write would block");
                        }
                        Err(e) => {
                            log::error!("Failed to write to socket; err = {:?}", e);
                        }
                    }
                }
                None => {
                    log::error!("Failed to get component implementation for component_id = {:?}", id);
                }
            }
        }
    }
}