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

use std::{
    io::{Read, Write},
    os::{
        fd::{FromRawFd, RawFd},
        unix::net::UnixStream,
    },
};

pub fn run<T: Component + 'static>(mut component: T) {
    // establish socket with parent
    let child_control_socket_fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(10) };
    let child_data_socket_fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(11) };
    let mut child_control_socket = unsafe { UnixStream::from_raw_fd(child_control_socket_fd) };
    let mut child_control_count: u8 = 0 ;
    let child_data_socket = unsafe { UnixStream::from_raw_fd(child_data_socket_fd) };
    child_data_socket.set_nonblocking(true).unwrap();

    // setup services
    let mut services = Services {
        communications: communications::Manager::new(child_data_socket),
    };

    // initialize the component
    component.init(&mut services);

    // acknowledge component init
    child_control_socket
        .write_all(&[b'k'])
        .expect("Failed to write to socket");

    #[cfg(feature = "instrument")]
    println!("Instrumentation enabled");

    #[cfg(feature = "instrument")]
    let mut times = Vec::new();

    // do work
    loop {
        let mut buf = [0; 2];
        child_control_socket
            .read_exact(&mut buf)
            .expect("Failed to read from socket");

        if buf[1] != child_control_count {
            panic!("Control count mismatch");
        }

        #[cfg(feature = "instrument")]
        {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;
            times.push((timestamp, child_control_count));
        }

        child_control_count += 1;

        match buf[0] {
            b'q' => break,
            b'r' => {
                services.communications.receive();
                component.run(&mut services);
                child_control_socket
                    .write_all(&[b'k'])
                    .expect("Failed to write to socket");
            }
            _ => (),
        }
    }

    #[cfg(feature = "instrument")]
    {
        println!("Instrumentation complete");

        // extract component type name
        let type_name = std::any::type_name::<T>();
        let instrument_file = format!("instrumentation_{}.csv", type_name);

        let mut writer = csv::Writer::from_path(instrument_file).expect("Failed to open file");
        for (i, time) in times.iter().enumerate() {
            let (time, count) = time;
            writer
                .serialize((i, time, count))
                .expect("Failed to write to file");
        }
    }
}
