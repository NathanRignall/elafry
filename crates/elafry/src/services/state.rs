use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

pub struct Manager {
    stream: UnixStream,
    data: Vec<u8>,
}

impl Manager {
    pub fn new(stream: UnixStream) -> Manager {
        Manager {
            stream,
            data: vec![],
        }
    }

    pub fn run(&mut self) {
        let mut stream = &self.stream;
        let mut length_buf = [0; 4];

        // loop for a number of times to read messages
        for _ in 0..10 {
            match stream.read_exact(&mut length_buf) {
                Ok(_) => {
                    // get length of message
                    let length = u32::from_be_bytes(length_buf);

                    // don't read if length is 0
                    if length == 0 {
                        continue;
                    }

                    // create buffer with length
                    let mut state_buf = vec![0; length as usize];

                    // read the message
                    stream.read_exact(&mut state_buf).unwrap();

                    self.data = state_buf;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock =>  break,
                Err(e) => panic!("encountered IO error: {}", e),
            }
        }
    }

    pub fn get_data(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data = data;

        // write the message
        let length = self.data.len() as u32;
        let mut length_buf = length.to_be_bytes().to_vec();
        length_buf.append(&mut self.data.clone());

        //if going to block, don't send message
        match self.stream.write_all(&length_buf) {
            Ok(_) => {},
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => log::error!("Write would block"),
            Err(e) => log::error!("encountered IO error: {}", e),
        }
    }
}
