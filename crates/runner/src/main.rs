use std::{io::{Read, Write}, os::{fd::{FromRawFd, RawFd}, unix::net::UnixStream}};

use libloading::{Library, Symbol};

fn main() {
    // establish socket with parent
    let fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(0) };
    let mut child_sock = unsafe { UnixStream::from_raw_fd(fd) };

    // send a message to the parent
    child_sock.write_all(b"ok").expect("Failed to write to socket");

    // get path to library using socket
    let mut buf = [0; 1024];
    let n = child_sock.read(&mut buf).expect("Failed to read from socket");
    let path = std::str::from_utf8(&buf[..n]).expect("Failed to convert to string");

    // load the library
    println!("** Loading library ** : {}", path);
    let mut component: Box<dyn elafry::Component> = unsafe {
        let lib = Library::new(path).expect("Failed to load library");
        let create_component: Symbol<unsafe fn() -> Box<dyn elafry::Component>> = lib
            .get(b"create_component")
            .expect("Failed to load function");
        create_component()
    };

    // acknowledge library load
    child_sock.write_all(b"ok").expect("Failed to write to socket");

    // get path of socket using socket
    let n = child_sock.read(&mut buf).expect("Failed to read from socket");
    let socket_path: &str = std::str::from_utf8(&buf[..n]).expect("Failed to convert to string");

    // acknowledge socket path
    child_sock.write_all(b"ok").expect("Failed to write to socket");

    // setup services
    println!("** Setting up services **");
    println!("** Socket path: {}", socket_path);
    let mut services = elafry::Services {
        communications: elafry::communications::Manager::new(socket_path),
    };

    // initialize the component
    println!("** Initializing component **");
    component.init(&mut services);

    // acknowledge component init
    child_sock.write_all(b"ok").expect("Failed to write to socket");

    // do work
    println!("** Doing work **");
    loop {
        let mut buf = [0; 2];
        child_sock.read_exact(&mut buf).expect("Failed to read from socket");
        services.communications.receive();
        component.run(&mut services);
        child_sock.write_all(b"ok").expect("Failed to write to socket");
    }

}
