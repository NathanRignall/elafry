pub mod communications;
pub mod messages;
pub mod state;

pub trait Component {
    fn new() -> Self;
    fn init(&mut self, services: &mut Services);
    fn run(&mut self, services: &mut Services);
    fn hello(&self);
}

pub struct Services {
    pub communications: communications::Manager,
}

use std::{io::{Read, Write}, os::{fd::{FromRawFd, RawFd}, unix::net::UnixStream}};

pub fn run<T: Component + 'static>(mut component: T) {
    // establish socket with parent
    let child_control_socket_fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(10) };
    let child_data_socket_fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(11) };
    let mut child_control_socket = unsafe { UnixStream::from_raw_fd(child_control_socket_fd) };
    let child_data_socket = unsafe { UnixStream::from_raw_fd(child_data_socket_fd) };
    child_data_socket.set_nonblocking(true).unwrap();

    // setup services
    let mut services = Services {
        communications: communications::Manager::new(child_data_socket),
    };

    // initialize the component
    component.init(&mut services);

    // acknowledge component init
    child_control_socket.write_all(&[b'k']).expect("Failed to write to socket");

    #[cfg(feature = "instrument")]
    let mut times = Vec::new();

    // do work
    loop {
        let mut buf = [0; 1];
        child_control_socket.read_exact(&mut buf).expect("Failed to read from socket");

        #[cfg(feature = "instrument")]
        {
            let start = std::time::Instant::now();
            times.push(start);
        }

        match buf[0] {
            b'q' => break,
            b'r' => {
                services.communications.receive();
                component.run(&mut services);
                child_control_socket.write_all(&[b'k']).expect("Failed to write to socket");
            }
            _ => (),
        }
    }
    
    #[cfg(feature = "instrument")]
    {
        let mut writer = csv::Writer::from_path("instrument.csv").expect("Failed to create file");
        for (i, time) in times.iter().enumerate() {
            writer.serialize((i, time.elapsed().as_nanos())).expect("Failed to write to file");
        }
    }

}