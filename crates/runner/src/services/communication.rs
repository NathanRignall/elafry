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
    pub fn new(port: u16) -> CommunicationService {
        let udp_socket = std::net::UdpSocket::bind(format!("0.0.0.0:{}", port)).unwrap();
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

            // don't read if component is not running
            if !component.run {
                continue;
            }

            // loop for a maximum of 5 times until no more data is available
            for _ in 0..5 {
                match &mut component.implentation {
                    Some(implentation) => {
                        // check if stoped
                        if !component.run {
                            break;
                        }

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
                                                    .entry(destination.channel_id)
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
                        break;
                    }
                }
            }
        }

        // check for data on udp socket
        for _ in 0..state.total_components() * 5 {
            let mut udp_buf = [0; 1024];

            match self.udp_socket.recv_from(&mut udp_buf) {
                Ok((_, address)) => {
                    // get length of message
                    let mut length_buf = [0; 4];
                    length_buf.copy_from_slice(&udp_buf[0..4]);
                    let length = u32::from_be_bytes(length_buf);

                    log::debug!(
                        "Received message from: {:?} and length: {}",
                        address,
                        length
                    );

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

            // check if stoped
            if !component.run {
                // clear buffer
                self.component_exit_buffer.remove(id);

                continue;
            }

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

#[cfg(test)]
mod tests {
    use super::*;

    // setup logging
    fn setup() {
        let _ = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("warn,info,debug,trace"),
        )
        .is_test(true)
        .try_init();
    }

    #[test]
    fn test_communication_component_to_runner() {
        setup();

        let (socket, child_socket) = std::os::unix::net::UnixStream::pair().unwrap();
        socket.set_nonblocking(true).unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut state = crate::global_state::GlobalState::new();
        let mut communication_service = CommunicationService::new(5000);
        let id = uuid::Uuid::new_v4();
        state.add_component(id, "test".to_string(), 1);
        state.add_component_implementation(
            id,
            crate::global_state::Implementation {
                data_socket: crate::global_state::Socket {
                    socket: socket.try_clone().unwrap(),
                    count: 0,
                },
                state_socket: crate::global_state::Socket {
                    socket: socket.try_clone().unwrap(),
                    count: 0,
                },
                child: std::process::Command::new("sleep")
                    .arg("1")
                    .spawn()
                    .unwrap(),
                child_pid: 1,
            },
        );

        communication_service.run(&mut state);

        state.add_route(
            RouteEndpoint {
                endpoint: Endpoint::Component(id),
                channel_id: 1,
            },
            RouteEndpoint {
                endpoint: Endpoint::Runner,
                channel_id: 2,
            },
        );

        // send message
        let message = Message {
            count: 1,
            channel_id: 1,
            data: vec![1, 2, 3],
        };
        let message_buf = Message::encode(&message);
        let length = message_buf.len() as u32;
        let mut length_buf = length.to_be_bytes().to_vec();
        length_buf.append(&mut message_buf.clone());
        let mut stream = child_socket.try_clone().unwrap();
        stream.write_all(&length_buf).unwrap();

        communication_service.run(&mut state);

        let message = state.get_message(2).unwrap();

        assert_eq!(message.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_communication_component_to_component() {
        setup();

        let (socket_1, child_socket_1) = std::os::unix::net::UnixStream::pair().unwrap();
        child_socket_1.set_nonblocking(true).unwrap();

        let (socket_2, child_socket_2) = std::os::unix::net::UnixStream::pair().unwrap();
        child_socket_2.set_nonblocking(true).unwrap();

        let mut state = crate::global_state::GlobalState::new();
        let mut communication_service = CommunicationService::new(5001);

        let id_1 = uuid::Uuid::new_v4();
        state.add_component(id_1, "test".to_string(), 1);
        state.add_component_implementation(
            id_1,
            crate::global_state::Implementation {
                data_socket: crate::global_state::Socket {
                    socket: socket_1.try_clone().unwrap(),
                    count: 0,
                },
                state_socket: crate::global_state::Socket {
                    socket: socket_1.try_clone().unwrap(),
                    count: 0,
                },
                child: std::process::Command::new("sleep")
                    .arg("1")
                    .spawn()
                    .unwrap(),
                child_pid: 1,
            },
        );

        let id_2 = uuid::Uuid::new_v4();
        state.add_component(id_2, "test".to_string(), 1);
        state.add_component_implementation(
            id_2,
            crate::global_state::Implementation {
                data_socket: crate::global_state::Socket {
                    socket: socket_2.try_clone().unwrap(),
                    count: 0,
                },
                state_socket: crate::global_state::Socket {
                    socket: socket_2.try_clone().unwrap(),
                    count: 0,
                },
                child: std::process::Command::new("sleep")
                    .arg("1")
                    .spawn()
                    .unwrap(),
                child_pid: 1,
            },
        );

        communication_service.run(&mut state);

        state.add_route(
            RouteEndpoint {
                endpoint: Endpoint::Component(id_1),
                channel_id: 3,
            },
            RouteEndpoint {
                endpoint: Endpoint::Component(id_2),
                channel_id: 4,
            },
        );

        // send message
        let message = Message {
            count: 1,
            channel_id: 3,
            data: vec![1, 2, 3],
        };
        let message_buf = Message::encode(&message);
        let length = message_buf.len() as u32;
        let mut length_buf = length.to_be_bytes().to_vec();
        length_buf.append(&mut message_buf.clone());
        let mut stream = child_socket_1.try_clone().unwrap();
        stream.write_all(&length_buf).unwrap();

        communication_service.run(&mut state);

        // receive message
        let mut stream = child_socket_2.try_clone().unwrap();
        let mut length_buf = [0; 4];
        stream.read_exact(&mut length_buf).unwrap();
        let length = u32::from_be_bytes(length_buf);
        let mut message_buf = vec![0; length as usize];
        stream.read_exact(&mut message_buf).unwrap();
        let message = Message::decode(&message_buf).unwrap();

        assert_eq!(message.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_communication_component_to_address() {
        setup();

        let (socket, child_socket) = std::os::unix::net::UnixStream::pair().unwrap();
        socket.set_nonblocking(true).unwrap();

        // create udp socket
        let udp_socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();

        let mut state = crate::global_state::GlobalState::new();
        let mut communication_service = CommunicationService::new(5002);
        let id = uuid::Uuid::new_v4();
        state.add_component(id, "test".to_string(), 1);
        state.add_component_implementation(
            id,
            crate::global_state::Implementation {
                data_socket: crate::global_state::Socket {
                    socket: socket.try_clone().unwrap(),
                    count: 0,
                },
                state_socket: crate::global_state::Socket {
                    socket: socket.try_clone().unwrap(),
                    count: 0,
                },
                child: std::process::Command::new("sleep")
                    .arg("1")
                    .spawn()
                    .unwrap(),
                child_pid: 1,
            },
        );

        communication_service.run(&mut state);

        state.add_route(
            RouteEndpoint {
                endpoint: Endpoint::Component(id),
                channel_id: 5,
            },
            RouteEndpoint {
                endpoint: Endpoint::Address(udp_socket.local_addr().unwrap()),
                channel_id: 6,
            },
        );

        // send message
        let message = Message {
            count: 1,
            channel_id: 5,
            data: vec![1, 2, 3],
        };
        let message_buf = Message::encode(&message);
        let length = message_buf.len() as u32;
        let mut length_buf = length.to_be_bytes().to_vec();
        length_buf.append(&mut message_buf.clone());
        let mut stream = child_socket.try_clone().unwrap();
        stream.write_all(&length_buf).unwrap();

        communication_service.run(&mut state);

        // receive message
        let mut udp_buf = [0; 1024];
        udp_socket.recv_from(&mut udp_buf).unwrap();
        let length_buf = &udp_buf[0..4];
        let length = u32::from_be_bytes(length_buf.try_into().unwrap());
        let message_buf = &udp_buf[4..length as usize + 4];
        let message = Message::decode(&message_buf.to_vec()).unwrap();

        assert_eq!(message.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_communication_address_to_runner() {
        setup();

        let udp_socket = std::net::UdpSocket::bind("127.0.0.0:0").unwrap();

        let mut state = crate::global_state::GlobalState::new();
        let mut communication_service = CommunicationService::new(5003);

        communication_service.run(&mut state);

        state.add_route(
            RouteEndpoint {
                endpoint: Endpoint::Address(udp_socket.local_addr().unwrap()),
                channel_id: 7,
            },
            RouteEndpoint {
                endpoint: Endpoint::Runner,
                channel_id: 8,
            },
        );

        // send message
        let message = Message {
            count: 1,
            channel_id: 7,
            data: vec![1, 2, 3],
        };
        let message_buf = Message::encode(&message);
        let length = message_buf.len() as u32;
        let mut length_buf = length.to_be_bytes().to_vec();
        length_buf.append(&mut message_buf.clone());
        udp_socket
            .send_to(&length_buf, udp_socket.local_addr().unwrap())
            .unwrap();

        communication_service.run(&mut state);

        let message = state.get_message(8).unwrap();

        assert_eq!(message.data, vec![1, 2, 3]);
    }
}
