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
    pub deadline: std::time::Duration,
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
                    // set the priority of the component to the highest
                    unsafe {
                        let ret = libc::sched_setscheduler(
                            implentation.child_pid,
                            libc::SCHED_FIFO,
                            &libc::sched_param { sched_priority: 99 },
                        );
                        if ret != 0 {
                            println!("Failed to set scheduler");
                        }
                    }

                    // resume the child
                    unsafe {
                        libc::kill(implentation.child_pid, libc::SIGCONT);
                    }

                    // sleep for the deadline
                    std::thread::sleep(frame.deadline);
                    

                    // // check if the component is still running
                    // let child_proc = procfs::process::Process::new(implentation.child_pid).unwrap();
                    // let child_state = child_proc.stat().unwrap().state;

                    // // if over deadline change priority to lowest
                    // if child_state != 'T' {
                    //     log::error!("Component over deadline {:?} {:?} {}", frame.component_id, child_state, frame.deadline.as_micros());
                        unsafe {
                            let ret = libc::sched_setscheduler(
                                implentation.child_pid,
                                libc::SCHED_IDLE,
                                &libc::sched_param { sched_priority: 0 },
                            );
                            if ret != 0 {
                                println!("Failed to set scheduler");
                            }
                        }
                    // }
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
