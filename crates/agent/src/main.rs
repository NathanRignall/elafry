mod agent;

fn main() {
    let mut agent = agent::Agent::new();

    // wait 1 second
    std::thread::sleep(std::time::Duration::from_secs(1));
    agent.demo_task1();

    // wait 25 seconds
    std::thread::sleep(std::time::Duration::from_secs(25));
    agent.demo_task2();

    // wait 200 seconds
    std::thread::sleep(std::time::Duration::from_secs(200));
}
