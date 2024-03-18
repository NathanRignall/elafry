use std::os::unix::net::UnixStream;

pub struct Manager {
    stream: UnixStream,
    send_count: u8,
    receive_count: u8,
}

impl Manager {
    pub fn new(stream: UnixStream) -> Manager {
        Manager {
            stream,
            send_count: 0,
            receive_count: 0,
        }
    }

    pub fn save_state(&mut self, _: Vec<u8>) {
        println!("Saving state");
        println!("{:?} {} {}", self.stream, self.send_count, self.receive_count);
    }

    pub fn load_state(&mut self) -> Vec<u8> {
        println!("Loading state");
        vec![]
    }
}
