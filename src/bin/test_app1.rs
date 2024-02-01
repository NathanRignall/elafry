
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

fn main() {
    elafry::wrapper::run(TestApp { count: 0 , velocity: Velocity { x: 0.0, y: 0.0, z: 0.0} }, "/tmp/sock-1", 100);
}

struct TestApp {
    count: u32,
    velocity: Velocity,
}

impl elafry::wrapper::App for TestApp {
    fn init(&mut self, _services: &mut elafry::wrapper::Services) {
        self.count = 0;
        println!("Starting up!");
    }

    fn run(&mut self, services: &mut elafry::wrapper::Services) {
        // do stuff with messages
        loop {
            let message = services.communications.get_message(2);
            match message {
                Some(message) => println!("Received message 1: {:?}", message),
                None => break,
            }
        }

        // do stuff
        self.count += 1;
        self.velocity.x += 1.0;
        self.velocity.y += 2.0;
        self.velocity.z += 3.0;

        // send message
        let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
        let velocity_buf = bincode::serialize(&self.velocity).unwrap();
        let message = elafry::wrapper::communications::Message {
            channel_id: 1,
            data: velocity_buf,
            count: self.count,
            timestamp: current_time.as_micros() as u64,
        };
        services.communications.send_message(message);
    }
}