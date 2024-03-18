use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

use crate::types::communication::Message;

pub struct Manager {
    stream: UnixStream,
    send_count: u8,
    receive_count: u8,
    messages: HashMap<u32, Vec<Message>>,
}

impl Manager {
    pub fn new(stream: UnixStream) -> Manager {
        Manager {
            stream,
            send_count: 0,
            receive_count: 0,
            messages: HashMap::new(),
        }
    }

    pub fn receive(&mut self) {
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
                    let message: Message = match bincode::deserialize(&message_buf) {
                        Ok(message) => message,
                        Err(e) => {
                            println!("Failed to deserialize message; err = {:?}", e);
                            continue;
                        }
                    };

                    // check count of message matches receive count
                    if message.count != self.receive_count {
                        log::error!(
                            "Received message with count {} but expected {}",
                            message.count,
                            self.receive_count
                        );
                        self.receive_count = message.count;
                    }

                    // increment receive count
                    self.receive_count += 1;

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

        // get timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        // form message
        let message = Message {
            channel_id,
            data,
            count: self.send_count,
            timestamp
        };

        // serialize message
        let message_buf = bincode::serialize(&message).unwrap();
        let length = message_buf.len() as u32;
        let mut length_buf = length.to_be_bytes().to_vec();
        length_buf.append(&mut message_buf.clone());

        // increment send count
        self.send_count += 1;

        // if going to block, don't send message
        match stream.write_all(&length_buf) {
            Ok(_) => (),
            Err(e) => panic!("encountered IO error: {}", e),
        }
    }
}
