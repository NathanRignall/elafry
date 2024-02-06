use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Message {
    pub channel_id: u32,
    pub data: Vec<u8>,
    pub count: u32,
    pub timestamp: u64,
}

pub struct Manager {
    stream: UnixStream,
    messages: HashMap<u32, Vec<Message>>,
}

impl Manager {
    pub fn new(path: &str) -> Manager {
        let stream = UnixStream::connect(path).unwrap();
        stream.set_nonblocking(true).unwrap();

        Manager { stream, messages: HashMap::new() }
    }

    pub fn receive(&mut self) {
        let mut stream = &self.stream;
        let mut length_buf = [0; 4];

        // loop for a maximum of 10 times until no more data is available
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
                    
                    // add message to hashmap
                    let channel_id = message.channel_id;

                    // check if channel_id exists in hashmap
                    if self.messages.contains_key(&channel_id) {
                        // append message to vector
                        let messages = self.messages.get_mut(&channel_id).unwrap();
                        messages.push(message);
                    } else {
                        // create new vector and add message
                        let mut messages = Vec::new();
                        messages.push(message);
                        self.messages.insert(channel_id, messages);
                    }
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

    pub fn send_message(&mut self, message: Message) {
        let mut stream = &self.stream;
        
        // serialize message
        let message_buf = bincode::serialize(&message).unwrap();
        let length = message_buf.len() as u32;
        let mut length_buf = length.to_be_bytes().to_vec();
        length_buf.append(&mut message_buf.clone());

        // if going to block, don't send message
        match stream.write_all(&length_buf) {
            Ok(_) => (),
            Err(e) => panic!("encountered IO error: {}", e),
        }
    }
    
}