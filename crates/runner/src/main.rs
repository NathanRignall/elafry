use crate::services::{
    communication::CommunicationService, management::ManagementService,
    scheduler::SchedulerService, state::StateService,
};

mod global_state;
mod services;

fn main() {
    env_logger::init();

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

    let mut global_state = global_state::GlobalState::new();

    let mut communication_service = CommunicationService::new(5000);
    let mut management_service = ManagementService::new("default.yaml".to_string());
    let mut scheduler_service = SchedulerService::new();
    let mut state_service = StateService::new();

    // frame index
    let mut last_sleep = std::time::Duration::from_micros(0);
    let mut last_duration = std::time::Duration::from_micros(0);
    let mut overruns = 0;
    let mut times = vec![];

    log::info!(
        "Starting runner loop with period {}us",
        global_state.schedule.period.as_micros()
    );

    loop {
        let last_time = std::time::Instant::now();

        times.push((
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            global_state.schedule.period.as_micros() as u64,
            last_sleep.as_micros() as u64,
            last_duration.as_micros() as u64,
            overruns,
            0,
        ));
        scheduler_service.run(&mut global_state);

        times.push((
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            global_state.schedule.period.as_micros() as u64,
            last_sleep.as_micros() as u64,
            last_duration.as_micros() as u64,
            overruns,
            1,
        ));
        communication_service.run(&mut global_state);

        times.push((
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            global_state.schedule.period.as_micros() as u64,
            last_sleep.as_micros() as u64,
            last_duration.as_micros() as u64,
            overruns,
            2,
        ));
        state_service.run(&mut global_state);

        times.push((
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            global_state.schedule.period.as_micros() as u64,
            last_sleep.as_micros() as u64,
            last_duration.as_micros() as u64,
            overruns,
            3,
        ));

        // if there is less than 100us left in the period, skip management
        let now = std::time::Instant::now();
        let duration = now.duration_since(last_time);
        if duration <= global_state.schedule.period - std::time::Duration::from_micros(100) {
            management_service.run(&mut global_state);
        }

        times.push((
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            global_state.schedule.period.as_micros() as u64,
            last_sleep.as_micros() as u64,
            last_duration.as_micros() as u64,
            overruns,
            4,
        ));

        // if done, break
        if global_state.get_done() {
            break;
        }

        // sleep for the rest of the period
        let now = std::time::Instant::now();
        let duration = now.duration_since(last_time);
        let mut sleep = std::time::Duration::from_micros(0);

        if duration <= global_state.schedule.period {
            sleep = global_state.schedule.period - duration;
            std::thread::sleep(sleep);
        } else {
            overruns += 1;
            log::error!(
                "Warning: loop took longer than period {}us - {}us",
                duration.as_micros(),
                last_sleep.as_micros()
            );
        }

        last_duration = duration;
        last_sleep = sleep;
    }

    let mut writer = csv::Writer::from_path("times.csv").expect("Failed to open file");
    for time in times.iter() {
        writer.serialize(time).expect("Failed to write to file");
    }

    log::info!("Runner loop complete");
}
