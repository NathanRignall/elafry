use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc, Mutex};

use command_fds::{CommandFdExt, FdMapping};
use uuid::Uuid;

use crate::global_state::Implementation;
use crate::services::management::ActionState;

pub enum NonBlockingImplementationData {
    AddComponent(AddComponentImplementation),
    RemoveComponent(RemoveComponentImplementation),
}

pub struct AddComponentImplementation {
    pub component_id: Uuid,
    pub component: String,
    pub core: usize,
}

pub struct RemoveComponentImplementation {
    pub component_id: Uuid,
    pub implementation: Implementation,
}

pub fn main(
    receiver: mpsc::Receiver<()>,
    non_blocking_actions: Arc<Mutex<Vec<NonBlockingImplementationData>>>,
    done_implement: Arc<Mutex<HashMap<Uuid, Implementation>>>,
    done_remove: Arc<Mutex<Vec<Uuid>>>,
) {
    loop {
        log::debug!("Waiting for signal");
        match receiver.recv() {
            Ok(_) => {
                log::info!("Received signal");

                // loop through all non-blocking actions
                let mut non_blocking_actions = non_blocking_actions.lock().unwrap();

                for action in non_blocking_actions.iter_mut() {
                    match action {
                        NonBlockingImplementationData::AddComponent(data) => {
                            // get the implementation
                            let implementation: Implementation =
                                add_component_implementation(data.component.clone(), data.core);

                            // add the implementation to the list of done implementations
                            let mut done_implement = done_implement.lock().unwrap();
                            done_implement.insert(data.component_id, implementation);
                        }
                        NonBlockingImplementationData::RemoveComponent(data) => {
                            // remove the implementation
                            remove_component_implementation(&mut data.implementation);

                            // add the component id to the list of done removes
                            let mut done_remove = done_remove.lock().unwrap();
                            done_remove.push(data.component_id);
                        }
                    }
                }

                // clear the list of non-blocking actions
                non_blocking_actions.clear();

                log::info!("Done processing signal");
            }
            Err(_) => {}
        }
    }
}

pub fn add_component_implementation(
    path: String,
    core: usize,
) -> crate::global_state::Implementation {
    log::trace!("BACKGROUND: Adding component {}", path);

    // create control and data sockets
    let (mut control_socket, child_control_socket) = UnixStream::pair().unwrap();
    let (data_socket, child_data_socket) = UnixStream::pair().unwrap();
    let (state_socket, child_state_socket) = UnixStream::pair().unwrap();
    data_socket.set_nonblocking(true).unwrap();
    state_socket.set_nonblocking(true).unwrap();

    // create fds for the child process
    let child_control_socket_fd = child_control_socket.into_raw_fd();
    let child_data_socket_fd = child_data_socket.into_raw_fd();
    let child_state_socket_fd = child_state_socket.into_raw_fd();

    // spawn the child process
    let binary_path = format!("target/release/{}", path);
    let mut command = Command::new(binary_path);
    command
        .fd_mappings(vec![
            FdMapping {
                child_fd: 10,
                parent_fd: unsafe { OwnedFd::from_raw_fd(child_control_socket_fd) },
            },
            FdMapping {
                child_fd: 11,
                parent_fd: unsafe { OwnedFd::from_raw_fd(child_data_socket_fd) },
            },
            FdMapping {
                child_fd: 12,
                parent_fd: unsafe { OwnedFd::from_raw_fd(child_state_socket_fd) },
            },
        ])
        .unwrap();
    // redirect the child's stderr to the parent's stderr
    let child = command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    // stop socket from being closed when it goes out of scope
    let _ = unsafe { UnixStream::from_raw_fd(child_control_socket_fd) };
    let _ = unsafe { UnixStream::from_raw_fd(child_data_socket_fd) };
    let _ = unsafe { UnixStream::from_raw_fd(child_state_socket_fd) };

    // use libc to set the process core affinity to specified core
    let mut cpu_set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::CPU_SET(core, &mut cpu_set);
        let ret = libc::sched_setaffinity(
            child.id() as libc::pid_t,
            std::mem::size_of_val(&cpu_set),
            &cpu_set,
        );
        if ret != 0 {
            log::error!("Failed to set affinity");
        }
    }

    // use libc to set the process sechdeuler to SCHEDULER FFIO
    unsafe {
        let ret = libc::sched_setscheduler(
            child.id() as libc::pid_t,
            libc::SCHED_FIFO,
            &libc::sched_param { sched_priority: 99 },
        );
        if ret != 0 {
            log::error!("Failed to set scheduler");
        }
    }

    // wait for the component to be ready
    let mut buffer = [0; 1];
    control_socket.read_exact(&mut buffer).unwrap();
    if buffer[0] != b'k' {
        panic!("Failed to start component");
    }

    log::trace!("BACKGROUND: Done adding component");

    // create the component implementation
    crate::global_state::Implementation {
        control_socket: crate::global_state::Socket {
            socket: control_socket,
            count: 0,
        },
        data_socket: crate::global_state::Socket {
            socket: data_socket,
            count: 0,
        },
        state_socket: crate::global_state::Socket {
            socket: state_socket,
            count: 0,
        },
        child,
    }
}

pub fn add_component(
    state: &mut crate::global_state::GlobalState,
    action_status: &mut ActionState,
    sender: Sender<()>,
    actions: Arc<Mutex<Vec<NonBlockingImplementationData>>>,
    done_implement: Arc<Mutex<HashMap<Uuid, Implementation>>>,
    data: elafry::types::configuration::AddComponentData,
) {
    log::debug!(
        "Adding component {} {:?}",
        data.component_id,
        *action_status
    );

    match *action_status {
        ActionState::Started => {
            // create the component
            state.add_component(data.component_id, data.component, data.core);

            // set the status to running
            *action_status = ActionState::Running;
        }
        ActionState::Running => {
            // try get a lock on the actions
            if let Ok(mut actions) = actions.try_lock() {
                // push the action to the actions vector
                actions.push(NonBlockingImplementationData::AddComponent(AddComponentImplementation {
                    component_id: data.component_id,
                    component: data.component,
                    core: data.core,
                }));

                // send signal to background thread
                sender.send(()).unwrap();

                // set the status to stopped
                *action_status = ActionState::Stopped;
            } else {
                log::warn!("Failed to get lock on actions");
            }
        }
        ActionState::Stopped => {
            // try get a lock on the done_implement
            if let Ok(mut done_implement) = done_implement.try_lock() {
                // pop the implementation from the done_implement hashmap
                if let Some(implementation) = done_implement.remove(&data.component_id) {
                    // put the implementation in the component
                    state.add_component_implementation(data.component_id, implementation);

                    // set the status to done
                    *action_status = ActionState::Completed;
                } else {
                    log::warn!("Component {} not done", data.component_id);
                }
            } else {
                log::warn!("Failed to get lock on done_implement");
            }
        }
        ActionState::Completed => {
            log::warn!("Should not be here");
        }
    }
}

pub fn remove_component_implementation(
    implementation: &mut Implementation,
) {
    log::trace!("BACKGROUND: Removing component");
    
    // wake the component
    implementation
    .control_socket
    .socket
    .write_all(&[b'w', implementation.control_socket.count])
    .unwrap();

    implementation.control_socket.count += 1;
    let mut buffer = [0; 1];
    implementation
        .control_socket
        .socket
        .read_exact(&mut buffer)
        .unwrap();

    // stop the component
    implementation
        .control_socket
        .socket
        .write_all(&[b'q', implementation.control_socket.count])
        .unwrap();

    // wait for the component to exit and kill
    implementation.child.wait().unwrap();
    implementation.child.kill().unwrap();

    log::trace!("BACKGROUND: Done removing component");
}

pub fn remove_component(
    state: &mut crate::global_state::GlobalState,
    action_status: &mut ActionState,
    sender: Sender<()>,
    actions: Arc<Mutex<Vec<NonBlockingImplementationData>>>,
    _done_remove: Arc<Mutex<Vec<Uuid>>>,
    data: elafry::types::configuration::RemoveComponentData,
) {
    log::debug!(
        "Removing component {} {:?}",
        data.component_id,
        *action_status
    );

    match *action_status {
        ActionState::Started => {
            // remove the component
            state.remove_component(data.component_id);

            // set the status to running
            *action_status = ActionState::Running;
        }
        ActionState::Running => {
            // try get a lock on the actions
            if let Ok(mut actions) = actions.try_lock() {
                // get the component from the state
                let component = state.get_component_mut(data.component_id).unwrap();

                // get the implementation from the component and set component implementation to None
                let implementation = component.implentation.take();

                // push the action to the actions vector
                actions.push(NonBlockingImplementationData::RemoveComponent(RemoveComponentImplementation {
                    component_id: data.component_id,
                    implementation: implementation.unwrap(),
                }));

                // send signal to background thread
                sender.send(()).unwrap();

                // set the status to stopped
                *action_status = ActionState::Stopped;
            } else {
                log::warn!("Failed to get lock on actions");
            }
        }
        ActionState::Stopped => {
            // try get a lock on the done_remove
            if let Ok(mut done_remove) = _done_remove.try_lock() {
                // get the uuid from the done_remove vector and remove it
                if let Some(uuid) = done_remove.pop() {
                    // remove the component implementation
                    state.remove_component_implementation(uuid);

                    // set the status to done
                    *action_status = ActionState::Completed;
                } else {
                    log::warn!("Component {} not done", data.component_id);
                }
            } else {
                log::warn!("Failed to get lock on done_remove");
            }

            *action_status = ActionState::Completed;
        }
        ActionState::Completed => {
            log::warn!("Should not be here");
        }
    }
}
