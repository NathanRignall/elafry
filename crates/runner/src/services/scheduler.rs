use std::io::{Read, Write};

pub struct Schedule {
    pub period: std::time::Duration,
    pub major_frames: Vec<MajorFrame>,
}

pub struct MajorFrame {
    pub minor_frames: Vec<MinorFrame>,
}

pub struct MinorFrame {
    pub component_id: uuid::Uuid,
}

pub struct SchedulerService {
    frame_index: usize,
}

impl SchedulerService {
    pub fn new() -> Self {
        SchedulerService { frame_index: 0 }
    }

    fn execute(&mut self, state: &mut crate::global_state::GlobalState) {
        // if there are no major frames, return
        if state.schedule.major_frames.is_empty() {
            log::warn!("No major frames");
            return;
        }

        // reset the frame index if it is out of bounds
        if self.frame_index >= state.schedule.major_frames.len() {
            self.frame_index = 0;
        }

        // get the current major frame
        let major_frame = &state.schedule.major_frames[self.frame_index];

        // run the minor frames
        for (_, frame) in major_frame.minor_frames.iter().enumerate() {
            // log::debug!("Running component {:?}", frame.component_id);

            let component = match state.components.get_mut(&frame.component_id) {
                Some(component) => component,
                None => {
                    log::error!("Component not found {:?}", frame.component_id);
                    continue;
                }
            };

            // if the component is not running, continue
            if !component.run {
                log::error!("Component not running {:?}", frame.component_id);
                continue;
            }

            match &mut component.implentation {
                Some(implentation) => {
                    // wake the component
                    implentation
                        .control_socket
                        .socket
                        .write_all(&[b'w', implentation.control_socket.count])
                        .unwrap();
                    implentation.control_socket.count += 1;
                    let mut buffer = [0; 1];
                    implentation
                        .control_socket
                        .socket
                        .read_exact(&mut buffer)
                        .unwrap();

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64;
                    component.times.push(timestamp);

                    // run the component
                    implentation
                        .control_socket
                        .socket
                        .write_all(&[b'r', implentation.control_socket.count])
                        .unwrap();
                    implentation.control_socket.count += 1;
                    let mut buffer = [0; 1];
                    implentation
                        .control_socket
                        .socket
                        .read_exact(&mut buffer)
                        .unwrap();

                    if buffer[0] != b'k' {
                        log::error!("Failed to run component");
                    }
                }
                None => {
                    log::error!("Component not started {:?}", frame.component_id);
                }
            }
        }
    }

    pub fn run(&mut self, state: &mut crate::global_state::GlobalState) {
        self.execute(state);
        self.frame_index += 1;
    }
}
