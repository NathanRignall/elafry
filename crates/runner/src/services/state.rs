use std::{collections::HashMap, io::Read};

pub struct StateService {
    component_state: HashMap<uuid::Uuid, Vec<u8>>,
}

impl StateService {
    pub fn new() -> Self {
        StateService {
            component_state: HashMap::new(),
        }
    }

    pub fn run(&mut self, state: &mut crate::global_state::GlobalState) {
        // check for data on components
        for (id, component) in state.components.iter_mut() {
            let mut length_buf = [0; 4];

            // loop for a maximum of 10 times until no more data is available
            for _ in 0..10 {
                match &mut component.implentation {
                    Some(implentation) => {
                        match implentation.data_socket.socket.read_exact(&mut length_buf) {
                            Ok(_) => {
                                // get length of message
                                let length = u32::from_be_bytes(length_buf);

                                // don't read if length is 0
                                if length == 0 {
                                    continue;
                                }

                                // create buffer with length
                                let state_buf = {
                                    let mut buf = vec![0; length as usize];
                                    match implentation.data_socket.socket.read_exact(&mut buf) {
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

                                // update or insert state
                                self.component_state.insert(id.clone(), state_buf);
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

    }
}