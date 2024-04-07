use elafry::Component;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct State {
    a: u8,
    count: u8,
}

struct DemoB {
    state: State,
}

impl elafry::Component for DemoB {
    fn new() -> Self {
        DemoB { state: State { a: 0, count: 0 } }
    }

    fn run(&mut self, services: &mut elafry::Services) {
        // do stuff with messages
        loop {
            let message = services.communication.get_message(1);
            match message {
                Some(message) => {
                    let new_a_state = message.data[0];

                    if new_a_state != self.state.a {
                        self.state.a = new_a_state;
                        services.communication.send_message(2, vec![self.state.a, 1]);
                    }
                }
                None => break,
            }
        }

        // increment count
        self.state.count += 1;

        // print state
        println!("B State count: {}", self.state.count);
    }

    fn save_state(&self) -> Vec<u8> {
        bincode::serialize(&self.state).unwrap()
    }

    fn load_state(&mut self, data: Vec<u8>) {
        self.state = bincode::deserialize(&data).unwrap();
    }

    fn reset_state(&mut self) {
        self.state = State { a: 0, count: 0 };
    }
}

fn main() {
    elafry::run(DemoB::new());
}
