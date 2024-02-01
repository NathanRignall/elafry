mod agent;

#[tokio::main]
async fn main() {
    let mut agent = agent::Agent::new();
    
    // wait 1 second
    std::thread::sleep(std::time::Duration::from_secs(1));
    agent.demo_task1().await;

    // wait 25 seconds
    std::thread::sleep(std::time::Duration::from_secs(75));
    agent.demo_task2().await;

    // wait 200 seconds
    std::thread::sleep(std::time::Duration::from_secs(200)); 
}

