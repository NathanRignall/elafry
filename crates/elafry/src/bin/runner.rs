use std::{io::{Read, Write}, os::{fd::{FromRawFd, RawFd}, unix::net::UnixStream}};
use libloading::{Library, Symbol};

fn main() {
    // establish socket with parent
    let child_control_socket_fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(0) };
    let child_data_socket_fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(1) };
    let mut child_control_socket = unsafe { UnixStream::from_raw_fd(child_control_socket_fd) };
    let child_data_socket = unsafe { UnixStream::from_raw_fd(child_data_socket_fd) };
    child_data_socket.set_nonblocking(true).unwrap();

    // send a message to the parent
    child_control_socket.write_all(&[b'k']).expect("Failed to write to socket");

    // get path to library using socket
    let mut buf = [0; 1024];
    let n = child_control_socket.read(&mut buf).expect("Failed to read from socket");
    let path = std::str::from_utf8(&buf[..n]).expect("Failed to convert to string");

    // load the library
    let mut component: Box<dyn elafry::Component> = unsafe {
        let lib = Library::new(path).expect("Failed to load library");
        let create_component: Symbol<unsafe fn() -> Box<dyn elafry::Component>> = lib
            .get(b"create_component")
            .expect("Failed to load function");
        create_component()
    };

    // acknowledge library load
    child_control_socket.write_all(&[b'k']).expect("Failed to write to socket");

    // setup services
    let mut services = elafry::Services {
        communications: elafry::communications::Manager::new(child_data_socket),
    };

    // initialize the component
    component.init(&mut services);
    println!("Component initialized");

    // acknowledge component init
    child_control_socket.write_all(&[b'k']).expect("Failed to write to socket");

    #[cfg(feature = "instrument")]
    let mut times = Vec::new();

    println!("Entering loop");

    // do work
    loop {
        let mut buf = [0; 1];
        child_control_socket.read_exact(&mut buf).expect("Failed to read from socket");

        #[cfg(feature = "instrument")]
        {
            let start = std::time::Instant::now();
            times.push(start);
        }

        println!("Received command: {:?}", buf[0] as char);

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
