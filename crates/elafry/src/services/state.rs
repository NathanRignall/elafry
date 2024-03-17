use std::os::unix::net::UnixStream;

pub struct Manager {
    stream: UnixStream,
}

impl Manager {
    pub fn new(stream: UnixStream) -> Manager {
        Manager { stream }
    }

    pub fn save_state(&mut self, _: Vec<u8>) {
        println!("Saving state");
    }

    pub fn load_state(&mut self) -> Vec<u8> {
        println!("Loading state");
        vec![]
    }
}
