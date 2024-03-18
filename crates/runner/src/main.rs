use std::{collections::HashMap, os::unix::net::UnixStream};

use elafry::types::communication::Message;
use services::{communication::RouteEndpoint, scheduler::Schedule};

use crate::services::{
    communication::CommunicationService, management::ManagementService,
    scheduler::SchedulerService, state::StateService,
};

mod services;

pub struct Component {
    run: bool,
    remove: bool,
    path: String,
    core: usize,
    implentation: Option<Implementation>,
    times: Vec<u64>,
}

pub struct Implementation {
    pub control_socket: Socket,
    pub data_socket: Socket,
    pub state_socket: Socket,
    pub child: std::process::Child,
}

pub struct Socket {
    pub socket: UnixStream,
    pub count: u8,
}

pub struct GlobalState {
    pub components: HashMap<uuid::Uuid, Component>,
    pub routes: HashMap<RouteEndpoint, RouteEndpoint>,
    pub schedule: Schedule,
    pub messages: HashMap<u32, Vec<Message>>,
    pub times: Vec<(u64, u64, u64, u64)>,
    pub done: bool,
}

fn main() {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    // use libc to set the process core affinity to specified core
    let mut cpu_set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::CPU_SET(1, &mut cpu_set);
        let ret = libc::sched_setaffinity(0, std::mem::size_of_val(&cpu_set), &cpu_set);
        if ret != 0 {
            log::error!("Failed to set affinity");
        }
    }

    // use libc to set the process sechdeuler to SCHEDULER FFIO
    unsafe {
        let ret = libc::sched_setscheduler(
            0,
            libc::SCHED_FIFO,
            &libc::sched_param { sched_priority: 99 },
        );
        if ret != 0 {
            log::error!("Failed to set scheduler");
        }
    }

    let mut state = GlobalState {
        components: HashMap::new(),
        routes: HashMap::new(),
        schedule: Schedule {
            period: std::time::Duration::from_secs(1),
            major_frames: vec![],
        },
        messages: HashMap::new(),
        times: vec![],
        done: false,
    };

    let mut communication_service = CommunicationService::new();
    let mut management_service = ManagementService::new("default.yaml".to_string());
    let mut scheduler_service = SchedulerService::new();
    let state_service = StateService::new();

    // frame index
    let period = std::time::Duration::from_micros(1_000_000 / 200 as u64);
    let mut last_sleep = std::time::Duration::from_micros(0);
    let mut last_duration = std::time::Duration::from_micros(0);
    let mut overruns = 0;
    let mut times = vec![];

    log::info!("Starting runner loop with period {}us", period.as_micros());

    loop {
        let last_time = std::time::Instant::now();

        times.push((
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            last_sleep.as_micros() as u64,
            last_duration.as_micros() as u64,
            overruns,
            0,
        ));
        scheduler_service.run(&mut state);

        times.push((
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            last_sleep.as_micros() as u64,
            last_duration.as_micros() as u64,
            overruns,
            1,
        ));
        communication_service.run(&mut state);

        times.push((
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            last_sleep.as_micros() as u64,
            last_duration.as_micros() as u64,
            overruns,
            2,
        ));
        management_service.run(&mut state);

        times.push((
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            last_sleep.as_micros() as u64,
            last_duration.as_micros() as u64,
            overruns,
            3,
        ));
        state_service.run(&mut state);

        // if done, break
        if state.done {
            break;
        }

        // sleep for the rest of the period
        let now = std::time::Instant::now();
        let duration = now.duration_since(last_time);
        let mut sleep = std::time::Duration::from_micros(0);

        if duration <= period {
            sleep = period - duration;
            std::thread::sleep(sleep);
        } else {
            overruns += 1;
            println!(
                "Warning: loop took longer than period {}us - {}us",
                duration.as_micros(),
                last_sleep.as_micros()
            );
        }

        last_duration = duration;
        last_sleep = sleep;
    }

    for (id, component) in state.components.iter_mut() {
        let instrument_file = format!("instrumentation_{}.csv", id);

        let mut writer = csv::Writer::from_path(instrument_file).expect("Failed to open file");
        for (i, time) in component.times.iter().enumerate() {
            writer
                .serialize((i, time))
                .expect("Failed to write to file");
        }
    }

    let mut writer = csv::Writer::from_path("times.csv").expect("Failed to open file");
    for time in times.iter() {
        writer.serialize(time).expect("Failed to write to file");
    }

    log::info!("Runner loop complete");
}
