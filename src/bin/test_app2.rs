use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

fn main() {
    elafry::wrapper::run(TestApp { count: 0, velocity: Velocity { x: 0.0, y: 0.0, z: 0.0}, update_time: std::time::Duration::from_secs(0), last_delay: 0 }, "/tmp/sock-2", 100);
}

struct TestApp {
    count: u32,
    velocity: Velocity,
    update_time: std::time::Duration,
    last_delay: u64,
}

impl elafry::wrapper::App for TestApp {
    fn init(&mut self, _services: &mut elafry::wrapper::Services) {
        self.count = 0;
        println!("Starting up!");
    }

    fn run(&mut self, services: &mut elafry::wrapper::Services) {
        // do stuff with messages
        let mut total_messages: u32 = 0;
        loop {
            let message = services.communications.get_message(1);
            match message {
                Some(message) => {
                    self.count += 1;
                    total_messages += 1;
                    
                    if self.count != message.count {
                        println!("--------COUNT MISMATCH-------- ({} != {})", self.count, message.count);
                        self.count = message.count;
                    }

                    let velocity: Velocity = match bincode::deserialize(&message.data) {
                        Ok(velocity) => velocity,
                        Err(e) => {
                            println!("Failed to deserialize velocity; err = {:?}", e);
                            continue;
                        }
                    };

                    self.velocity = velocity;
                    self.update_time = std::time::Duration::from_micros(message.timestamp as u64);
                }
                None => {
                    println!("Total messages: {}", total_messages);
                    break;
                }
            }
        }

        // do stuff
        // println!("Velocity: {:?}", self.velocity);

        let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
        let duration = current_time.as_micros() as u64 - self.update_time.as_micros() as u64;
        // if self.last_delay < duration {
        //     print!("----------DELAY INCREASED!----------");
        // }
        self.last_delay = duration;
        println!("Delay in seconds: {}", duration as f64 / 1_000_000.0);
    }
}