use std::{
    collections::HashMap,
    io::{Read, Write},
    net::SocketAddr,
};

use elafry::types::communication::Message;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct RouteEndpoint {
    pub endpoint: Endpoint,
    pub channel_id: u32,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum Endpoint {
    Component(uuid::Uuid),
    Address(SocketAddr),
    Runner,
}

pub struct CommunicationService {
    udp_socket: std::net::UdpSocket,
    component_exit_buffer: HashMap<uuid::Uuid, Vec<Message>>,
    address_exit_buffer: HashMap<SocketAddr, Vec<Message>>,
}

impl CommunicationService {
    pub fn new() -> Self {
        let udp_socket = std::net::UdpSocket::bind("0.0.0.0:5000").unwrap();
        udp_socket.set_nonblocking(true).unwrap();

        CommunicationService {
            udp_socket,
            component_exit_buffer: HashMap::new(),
            address_exit_buffer: HashMap::new(),
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
                                let message_buf = {
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

                                // deserialize message
                                let message: Message = match Message::decode(&message_buf) {
                                    Some(message) => message,
                                    None => {
                                        log::error!("Failed to decode message");
                                        continue;
                                    }
                                };

                                let destination: Option<RouteEndpoint>;
                                {
                                    destination = state
                                        .routes
                                        .get(&RouteEndpoint {
                                            endpoint: Endpoint::Component(*id),
                                            channel_id: message.channel_id,
                                        })
                                        .cloned();
                                }

                                match destination {
                                    Some(destination) => {
                                        // insert the message into the correct buffer
                                        match destination.endpoint {
                                            Endpoint::Component(id) => {
                                                match self.component_exit_buffer.get_mut(&id) {
                                                    Some(buffer) => {
                                                        buffer.push(message);
                                                    }
                                                    None => {
                                                        self.component_exit_buffer
                                                            .insert(id, vec![message]);
                                                    }
                                                }
                                            }
                                            Endpoint::Address(address) => {
                                                match self.address_exit_buffer.get_mut(&address) {
                                                    Some(buffer) => {
                                                        buffer.push(message);
                                                    }
                                                    None => {
                                                        self.address_exit_buffer
                                                            .insert(address, vec![message]);
                                                    }
                                                }
                                            }
                                            Endpoint::Runner => {
                                                state
                                                    .messages
                                                    .entry(message.channel_id)
                                                    .or_insert(Vec::new())
                                                    .push(message);
                                            }
                                        }
                                    }
                                    None => {
                                        log::warn!(
                                            "No route found for: {:?}",
                                            RouteEndpoint {
                                                endpoint: Endpoint::Component(*id),
                                                channel_id: message.channel_id,
                                            }
                                        );
                                    }
                                }
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

        // check for data on udp socket
        for _ in 0..state.total_components() * 10 {
            let mut udp_buf = [0; 1024];

            match self.udp_socket.recv_from(&mut udp_buf) {
                Ok((_, address)) => {
                    // get length of message
                    let mut length_buf = [0; 4];
                    length_buf.copy_from_slice(&udp_buf[0..4]);
                    let length = u32::from_be_bytes(length_buf);

                    log::debug!("Received message from: {:?} and length: {}", address, length);

                    // don't read if length is 0
                    if length == 0 {
                        continue;
                    }

                    // create buffer with length
                    let message_buf = {
                        let mut buf = vec![0; length as usize];
                        buf.copy_from_slice(&udp_buf[4..length as usize + 4]);
                        buf
                    };

                    // deserialize message
                    let message: Message = match Message::decode(&message_buf) {
                        Some(message) => message,
                        None => {
                            log::error!("Failed to decode message");
                            continue;
                        }
                    };

                    let destination: Option<RouteEndpoint>;
                    {
                        destination = state
                            .routes
                            .get(&RouteEndpoint {
                                endpoint: Endpoint::Address(address),
                                channel_id: message.channel_id,
                            })
                            .cloned();
                    }

                    match destination {
                        Some(destination) => {
                            // insert the message into the correct buffer
                            match destination.endpoint {
                                Endpoint::Component(id) => {
                                    match self.component_exit_buffer.get_mut(&id) {
                                        Some(buffer) => {
                                            buffer.push(message);
                                        }
                                        None => {
                                            self.component_exit_buffer.insert(id, vec![message]);
                                        }
                                    }
                                }
                                Endpoint::Address(address) => {
                                    match self.address_exit_buffer.get_mut(&address) {
                                        Some(buffer) => {
                                            buffer.push(message);
                                        }
                                        None => {
                                            self.address_exit_buffer.insert(address, vec![message]);
                                        }
                                    }
                                }
                                Endpoint::Runner => {
                                    state
                                        .messages
                                        .entry(message.channel_id)
                                        .or_insert(Vec::new())
                                        .push(message);
                                }
                            }
                        }
                        None => {
                            log::warn!(
                                "No route found for: {:?}",
                                RouteEndpoint {
                                    endpoint: Endpoint::Address(address),
                                    channel_id: message.channel_id,
                                }
                            );
                        }
                    }
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

        // check for data to send to clear the exit component buffer
        for (id, component) in state.components.iter_mut() {
            let messages = match self.component_exit_buffer.get(id) {
                Some(messages) => messages,
                None => {
                    continue;
                }
            };

            for message in messages.iter() {
                let message_buf = Message::encode(message);

                let length = message_buf.len() as u32;

                let length_buf = length.to_be_bytes();

                let combined_buf = [length_buf.to_vec(), message_buf].concat();

                match &mut component.implentation {
                    Some(implentation) => {
                        match implentation.data_socket.socket.write_all(&combined_buf) {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Failed to write to socket; err = {:?}", e);
                            }
                        }
                    }
                    None => {
                        log::error!("No implementation found for component: {:?}", id);
                        continue;
                    }
                }
            }

            self.component_exit_buffer.remove(id);
        }

        // check for data to send to clear the exit address buffer
        for (address, messages) in self.address_exit_buffer.iter() {
            for message in messages.iter() {
                let message_buf = Message::encode(message);

                let length = message_buf.len() as u32;

                let length_buf = length.to_be_bytes();

                let combined_buf = [length_buf.to_vec(), message_buf].concat();

                match self.udp_socket.send_to(&combined_buf, address) {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Failed to write to socket; err = {:?}", e);
                    }
                }
            }
        }
        self.address_exit_buffer.clear();
    }
}
