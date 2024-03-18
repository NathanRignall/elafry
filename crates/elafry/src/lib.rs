pub mod services;
pub mod types;

pub trait Component {
    fn new() -> Self;
    fn init(&mut self, services: &mut Services);
    fn run(&mut self, services: &mut Services);
}

pub struct Services {
    pub communication: services::communication::Manager,
    pub state: services::state::Manager,
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
    let child_state_socket_fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(12) };

    // set up control socket
    let mut child_control_socket = unsafe { UnixStream::from_raw_fd(child_control_socket_fd) };
    child_control_socket.set_nonblocking(false).unwrap();
    let mut child_control_count: u8 = 0;

    // set up data socket
    let child_data_socket = unsafe { UnixStream::from_raw_fd(child_data_socket_fd) };
    child_data_socket.set_nonblocking(true).unwrap();

    // set up state socket
    let child_state_socket = unsafe { UnixStream::from_raw_fd(child_state_socket_fd) };
    child_state_socket.set_nonblocking(true).unwrap();
    
    // setup services
    let mut services = Services {
        communication: services::communication::Manager::new(child_data_socket),
        state: services::state::Manager::new(child_state_socket),
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
        // set to non-blocking mode
        child_control_socket.set_nonblocking(false).unwrap();

        let mut buf = [0; 2];
        child_control_socket
            .read_exact(&mut buf)
            .expect("Failed to read from socket");
        child_control_socket
            .write_all(&[b'k'])
            .expect("Failed to write to socket");

        child_control_count += 1;

        // set to blocking mode
        child_control_socket.set_nonblocking(true).unwrap();
        loop {
            match child_control_socket.read_exact(&mut buf) {
                Ok(_) => break,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(e) => panic!("Failed to read from socket: {}", e),
            }
        }

        if buf[1] != child_control_count {
            println!(
                "Control count mismatch ({} != {})",
                buf[1], child_control_count
            );
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
            b'q' => {
                child_control_socket
                    .write_all(&[b'k'])
                    .expect("Failed to write to socket");
                break;
            }
            b'r' => {
                services.communication.receive();
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
