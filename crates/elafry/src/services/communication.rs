use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

use crate::types::communication::Message;

pub struct State {
    send_count: u8,
    receive_count: u8,
}

pub struct Manager {
    stream: UnixStream,
    state: State,
    messages: HashMap<u32, Vec<Message>>,
}

impl Manager {
    pub fn new(stream: UnixStream) -> Manager {
        Manager {
            stream,
            state: State {
                send_count: 0,
                receive_count: 0,
            },
            messages: HashMap::new(),
        }
    }

    pub fn run(&mut self) {
        let mut stream = &self.stream;
        let mut length_buf = [0; 4];

        // loop for a number of times to read messages
        for _ in 0..1000 {
            match stream.read_exact(&mut length_buf) {
                Ok(_) => {
                    // get length of message
                    let length = u32::from_be_bytes(length_buf);

                    // don't read if length is 0
                    if length == 0 {
                        continue;
                    }

                    // create buffer with length
                    let mut message_buf = vec![0; length as usize];

                    // read the message
                    stream.read_exact(&mut message_buf).unwrap();

                    // deserialize message
                    let message: Message = match Message::decode(&message_buf) {
                        Some(message) => message,
                        None => {
                            log::error!("Failed to decode message");
                            continue;
                        }
                    };

                    // check count of message matches receive count
                    if message.count != self.state.receive_count {
                        log::error!(
                            "Received message with count {} but expected {}",
                            message.count,
                            self.state.receive_count
                        );
                        self.state.receive_count = message.count;
                    }

                    // increment receive count
                    self.state.receive_count += 1;

                    // add message to hashmap
                    let channel_id = message.channel_id;
                    let messages = self.messages.entry(channel_id).or_insert(vec![]);
                    messages.push(message);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("encountered IO error: {}", e),
            }
        }

        // for each channel_id in hashmap, sort messages by count in descending order
        for (_, messages) in self.messages.iter_mut() {
            messages.sort_by(|a, b| b.count.cmp(&a.count));
        }
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

    pub fn send_message(&mut self, channel_id: u32, data: Vec<u8>) {
        let mut stream = &self.stream;

        // form message
        let message = Message {
            channel_id,
            data,
            count: self.state.send_count,
        };

        // serialize message
        let message_buf = Message::encode(&message);
        let length = message_buf.len() as u32;
        let mut length_buf = length.to_be_bytes().to_vec();
        length_buf.append(&mut message_buf.clone());

        // increment send count
        self.state.send_count += 1;

        // if going to block, don't send message
        match stream.write_all(&length_buf) {
            Ok(_) => (),
            Err(e) => panic!("encountered IO error: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // setup logging
    fn setup() {
        let _ = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("error,warn,info,debug,trace"),
        )
        .is_test(true)
        .try_init();
    }

    #[test]
    fn test_communication() {
        setup();

        let (socket, child_socket) = UnixStream::pair().unwrap();
        socket.set_nonblocking(true).unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut manager_1 = Manager::new(socket);
        let mut manager_2 = Manager::new(child_socket);

        manager_1.send_message(1, vec![7, 8, 9]);
        manager_1.send_message(1, vec![4, 5, 6]);

        manager_1.send_message(2, vec![10, 11, 12]);
        manager_2.run();

        let message = manager_2.get_message(1).unwrap();
        assert_eq!(message.data, vec![7, 8, 9]);

        let message = manager_2.get_message(1).unwrap();
        assert_eq!(message.data, vec![4, 5, 6]);

        let message = manager_2.get_message(2).unwrap();
        assert_eq!(message.data, vec![10, 11, 12]);

        let message = manager_2.get_message(1);
        assert_eq!(message, None);

        let message = manager_2.get_message(2);
        assert_eq!(message, None);
    }

    #[test]
    fn test_communication_zero_length() {
        setup();

        let (socket, child_socket) = UnixStream::pair().unwrap();
        socket.set_nonblocking(true).unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut manager = Manager::new(child_socket);

        // put bad data on socket
        let mut stream = &socket;
        let length_buf = [0, 0, 0, 0];
        stream.write_all(&length_buf).unwrap();

        manager.run();

        let message = manager.get_message(1);
        assert_eq!(message, None);
    }

    #[test]
    fn test_communication_short_length() {
        setup();

        let (socket, child_socket) = UnixStream::pair().unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut manager = Manager::new(child_socket);

        // put bad data on socket
        let mut stream = &socket;
        let length_buf = [0, 0, 0, 1];
        let data = [1];
        stream.write_all(&length_buf).unwrap();
        stream.write_all(&data).unwrap();

        manager.run();

        let message = manager.get_message(1);
        assert_eq!(message, None);
    }

    #[test]
    fn test_communication_bad_data() {
        setup();

        let (socket, child_socket) = UnixStream::pair().unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut manager = Manager::new(child_socket);

        // put bad data on socket
        let mut stream = &socket;
        let length_buf = [0, 0, 0, 5];
        let data = [1, 2, 3, 4, 5];
        stream.write_all(&length_buf).unwrap();
        stream.write_all(&data).unwrap();

        manager.run();

        let message = manager.get_message(1);
        assert_eq!(message, None);
    }

    #[test]
    #[should_panic]
    fn test_communication_bad_socket() {
        setup();

        let (socket, child_socket) = UnixStream::pair().unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut manager = Manager::new(child_socket);

        // close socket
        drop(socket);

        manager.run();
    }

    #[test]
    #[should_panic]
    fn test_communication_bad_socket_write() {
        setup();
        
        let (socket, child_socket) = UnixStream::pair().unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut manager = Manager::new(child_socket);

        // close socket
        drop(socket);

        manager.send_message(1, vec![1, 2, 3]);
    }
}