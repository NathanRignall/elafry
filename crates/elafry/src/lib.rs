pub mod services;
pub mod types;

pub trait Component {
    fn new() -> Self;
    fn run(&mut self, services: &mut Services);
    fn load_state(&mut self, data: Vec<u8>);
    fn save_state(&self) -> Vec<u8>;
    fn reset_state(&mut self);
}

pub struct Services {
    pub communication: services::communication::Manager,
    pub state: services::state::Manager,
}

use std::os::{
    fd::{FromRawFd, RawFd},
    unix::net::UnixStream,
};

pub fn run<T: Component + 'static>(mut component: T) {
    env_logger::init();

    log::info!("Starting component");

    // establish socket with parent
    let child_data_socket_fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(10) };
    let child_state_socket_fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(11) };

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
    component.reset_state();

    // save the initial state
    services.state.set_data(component.save_state());

    #[cfg(feature = "instrument")]
    log::debug!("Instrumentation enabled");

    #[cfg(feature = "instrument")]
    let mut times = Vec::new();

    // do work
    loop {
        // log::info!("Running component");

        // suspend self
        let pid = unsafe { libc::getpid() };
        if unsafe { libc::kill(pid, libc::SIGSTOP) } != 0 {
            panic!("Failed to suspend child");
        }

        // log::info!("Resumed");

        #[cfg(feature = "instrument")]
        {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;
            times.push(timestamp);
        }

        // run the services
        services.state.run();
        services.communication.run();

        // run the component
        component.load_state(services.state.get_data());
        component.run(&mut services);
        services.state.set_data(component.save_state());

        // log::info!("Component done");
    }
}
