use std::os::unix::net::UnixStream;

pub struct Manager {
    stream: UnixStream,
    send_count: u8,
    receive_count: u8,
    data: Vec<u8>,
}

impl Manager {
    pub fn new(stream: UnixStream) -> Manager {
        Manager {
            stream,
            send_count: 0,
            receive_count: 0,
            data: vec![],
        }
    }

    pub fn run(&self) {
        // println!("Running state manager");
    }

    pub fn get_data(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data = data;
    }
}
