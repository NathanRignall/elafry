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
            Err(e) => panic!("encountered IO error: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // setup logging
    fn setup() {
        let _ = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("warn,info,debug,trace"),
        )
        .is_test(true)
        .try_init();
    }
    
    #[test]
    fn test_manager_get_data() {
        setup();

        let (socket, child_socket) = UnixStream::pair().unwrap();
        socket.set_nonblocking(true).unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut manager = Manager::new(child_socket);

        // write data to child socket
        let mut stream = &socket;
        let length = 4u32.to_be_bytes().to_vec();
        let data = vec![1, 2, 3, 4];
        stream.write_all(&length).unwrap();
        stream.write_all(&data).unwrap();

        // run manager
        manager.run();

        // check data
        assert_eq!(manager.get_data(), data);

        // change data
        assert_eq!(manager.get_data(), data);

        // write data to child socket
        let mut stream = &socket;
        let length = 4u32.to_be_bytes().to_vec();
        let data = vec![5, 6, 7, 8];
        stream.write_all(&length).unwrap();
        stream.write_all(&data).unwrap();

        // run manager
        manager.run();

        // check data
        assert_eq!(manager.get_data(), data);

        // change data
        assert_eq!(manager.get_data(), data);
    }

    #[test]
    fn test_manager_set_data() {
        setup();

        let (socket, child_socket) = UnixStream::pair().unwrap();
        socket.set_nonblocking(true).unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut manager_1 = Manager::new(socket);
        let mut manager_2 = Manager::new(child_socket);

        // check empty data
        assert_eq!(manager_2.get_data(), vec![]);

        // set data
        let data = vec![1, 2, 3, 4];
        manager_1.set_data(data.clone());

        // run managers
        manager_1.run();
        manager_2.run();

        // check data
        assert_eq!(manager_2.get_data(), data);
        
        // set data
        let data = vec![5, 6, 7, 8];
        manager_1.set_data(data.clone());

        // run managers
        manager_1.run();
        manager_2.run();

        // check data
        assert_eq!(manager_2.get_data(), data);
    }

    #[test]
    fn test_manager_zero_length() {
        setup();

        let (socket, child_socket) = UnixStream::pair().unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut manager = Manager::new(child_socket);

        // put bad data on socket
        let mut stream = &socket;
        let length_buf = [0, 0, 0, 0];
        stream.write_all(&length_buf).unwrap();

        manager.run();

        let data = manager.get_data();
        assert_eq!(data, vec![]);
    }

    #[test]
    #[should_panic]
    fn test_manager_bad_socket_get_data() {
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
    fn test_manager_bad_socket_set_data() {
        setup();

        let (socket, child_socket) = UnixStream::pair().unwrap();
        child_socket.set_nonblocking(true).unwrap();

        let mut manager = Manager::new(child_socket);

        // close socket
        drop(socket);

        manager.set_data(vec![1, 2, 3]);
    }
}