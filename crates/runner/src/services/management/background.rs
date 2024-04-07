use std::collections::HashMap;
// use std::io::{Read};
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc, Mutex};

use command_fds::{CommandFdExt, FdMapping};
use uuid::Uuid;

use crate::global_state::{Implementation, StateSyncStatus};
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
                log::debug!("Received signal");

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

                log::debug!("Done processing signal");
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
    let (data_socket, child_data_socket) = UnixStream::pair().unwrap();
    data_socket.set_nonblocking(true).unwrap();

    let (state_socket, child_state_socket) = UnixStream::pair().unwrap();
    state_socket.set_nonblocking(true).unwrap();

    // create fds for the child process
    let child_data_socket_fd = child_data_socket.into_raw_fd();
    let child_state_socket_fd = child_state_socket.into_raw_fd();

    // spawn the child process
    let mut command = Command::new(path);
    command
        .fd_mappings(vec![
            FdMapping {
                child_fd: 10,
                parent_fd: unsafe { OwnedFd::from_raw_fd(child_data_socket_fd) },
            },
            FdMapping {
                child_fd: 11,
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
    std::thread::sleep(std::time::Duration::from_micros(100));

    log::trace!("BACKGROUND: Done adding component");

    let pid = child.id() as libc::pid_t;

    // create the component implementation
    crate::global_state::Implementation {
        data_socket: crate::global_state::Socket {
            socket: data_socket,
            count: 0,
        },
        state_socket: crate::global_state::Socket {
            socket: state_socket,
            count: 0,
        },
        child,
        child_pid: pid,
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
                actions.push(NonBlockingImplementationData::AddComponent(
                    AddComponentImplementation {
                        component_id: data.component_id,
                        component: data.component,
                        core: data.core,
                    },
                ));

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

fn remove_component_implementation(implementation: &mut Implementation) {
    log::trace!("BACKGROUND: Removing component");

    // send signal to child process to stop
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
                actions.push(NonBlockingImplementationData::RemoveComponent(
                    RemoveComponentImplementation {
                        component_id: data.component_id,
                        implementation: implementation.unwrap(),
                    },
                ));

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

                    // remove the component from the state
                    state.remove_component(uuid);

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

pub fn wait_state_sync(
    state: &mut crate::global_state::GlobalState,
    action_status: &mut ActionState,
    data: elafry::types::configuration::WaitStateSyncData,
) {
    log::debug!("Syncing state {} {:?}", data.state_sync_id, *action_status);

    match *action_status {
        ActionState::Started => {
            // create the state sync
            state.set_state_sync_status(data.state_sync_id, StateSyncStatus::Started);

            // set the status to running
            *action_status = ActionState::Running;
        }
        ActionState::Running => {
            // wait for the state to be synced
            let state_sync = state.get_state_sync_status(data.state_sync_id);

            // if the state is synced
            match state_sync {
                StateSyncStatus::Synced => {
                    // set the status to completed
                    *action_status = ActionState::Completed;
                }
                _ => {
                    log::warn!("State not synced");
                }
            }
        }
        ActionState::Stopped => {
            log::warn!("Should not be here");
        }
        ActionState::Completed => {
            log::warn!("Should not be here");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::global_state::Socket;

    use super::*;
    use std::sync::mpsc::channel;
    use std::thread;

    #[test]
    fn test_add_component_implementation() {
        let path = "ls";
        let core = 0;

        let implementation = add_component_implementation(path.to_string(), core);

        assert_eq!(implementation.data_socket.count, 0);
        assert_eq!(implementation.state_socket.count, 0);
    }

    #[test]
    fn test_remove_component_implementation() {
        let path = "ls";
        let core = 0;

        let mut implementation = add_component_implementation(path.to_string(), core);

        remove_component_implementation(&mut implementation);
    }

    #[test]
    fn test_add_component() {
        let (sender, receiver) = channel();
        let actions = Arc::new(Mutex::new(Vec::new()));
        let done_implement = Arc::new(Mutex::new(HashMap::new()));
        let mut state = crate::global_state::GlobalState::new();
        let mut action_status = ActionState::Started;

        // start the background thread
        let actions_clone = actions.clone();
        let done_implement_clone = done_implement.clone();
        thread::spawn(move || {
            main(
                receiver,
                actions_clone,
                done_implement_clone,
                Arc::new(Mutex::new(Vec::new())),
            );
        });

        let data = elafry::types::configuration::AddComponentData {
            component_id: Uuid::new_v4(),
            component: "ls".to_string(),
            core: 0,
            version: "0.1.0".to_string(),
        };

        add_component(
            &mut state,
            &mut action_status,
            sender.clone(),
            actions.clone(),
            done_implement.clone(),
            data.clone(),
        );

        assert_eq!(action_status, ActionState::Running);

        add_component(
            &mut state,
            &mut action_status,
            sender.clone(),
            actions.clone(),
            done_implement.clone(),
            data.clone(),
        );

        assert_eq!(action_status, ActionState::Stopped);

        // wait for the background thread to finish
        thread::sleep(std::time::Duration::from_secs(1));

        add_component(
            &mut state,
            &mut action_status,
            sender.clone(),
            actions.clone(),
            done_implement.clone(),
            data.clone(),
        );

        assert_eq!(action_status, ActionState::Completed);

        // check if the component was added to the state
        assert!(state.get_component(data.component_id).is_some());
        assert!(state
            .get_component(data.component_id)
            .unwrap()
            .implentation
            .is_some());
    }

    #[test]
    fn test_remove_component() {
        let (sender, receiver) = channel();
        let actions = Arc::new(Mutex::new(Vec::new()));
        let done_remove = Arc::new(Mutex::new(Vec::new()));
        let mut state = crate::global_state::GlobalState::new();
        let mut action_status = ActionState::Started;

        // create a dummy component on the state
        let id = uuid::Uuid::new_v4();
        let path = "path".to_string();
        let core = 0;
        let implementation = Implementation {
            data_socket: Socket {
                socket: UnixStream::pair().unwrap().0,
                count: 0,
            },
            state_socket: Socket {
                socket: UnixStream::pair().unwrap().0,
                count: 0,
            },
            child: std::process::Command::new("ls").spawn().unwrap(),
            child_pid: 0,
        };

        state.add_component(id, path.clone(), core);
        state.add_component_implementation(id, implementation);

        // start the background thread
        let actions_clone = actions.clone();
        let done_remove_clone = done_remove.clone();
        thread::spawn(move || {
            main(
                receiver,
                actions_clone,
                Arc::new(Mutex::new(HashMap::new())),
                done_remove_clone,
            );
        });

        let data = elafry::types::configuration::RemoveComponentData { component_id: id };

        remove_component(
            &mut state,
            &mut action_status,
            sender.clone(),
            actions.clone(),
            done_remove.clone(),
            data.clone(),
        );

        assert_eq!(action_status, ActionState::Running);

        remove_component(
            &mut state,
            &mut action_status,
            sender.clone(),
            actions.clone(),
            done_remove.clone(),
            data.clone(),
        );

        assert_eq!(action_status, ActionState::Stopped);

        // wait for the background thread to finish
        thread::sleep(std::time::Duration::from_secs(1));

        remove_component(
            &mut state,
            &mut action_status,
            sender.clone(),
            actions.clone(),
            done_remove.clone(),
            data.clone(),
        );

        assert_eq!(action_status, ActionState::Completed);

        // check if the component was removed from the state
        assert_eq!(state.total_components(), 1);
        assert_eq!(state.get_component(id).unwrap().remove, true);
        assert_eq!(state.get_component(id).unwrap().run, false);
        assert_eq!(
            state.get_component(id).unwrap().implentation.is_none(),
            true
        );
    }
}
