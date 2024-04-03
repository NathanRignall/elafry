use elafry::Component;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct State {
    a: u8,
    b: u8,
}

struct DemoB {
    state: State,
}

impl elafry::Component for DemoB {
    fn new() -> Self {
        DemoB { state: State { a: 0, b: 0 } }
    }

    fn run(&mut self, services: &mut elafry::Services) {
        // do stuff with messages
        loop {
            let message = services.communication.get_message(1);
            match message {
                Some(message) => {
                    let new_a_state = message.data[0];
                    let new_b_state = message.data[1];

                    if new_a_state != self.state.a {
                        self.state.a = new_a_state;
                    }
                    if new_b_state != self.state.b {
                        self.state.b = new_b_state;
                    }

                }
                None => break,
            }

            let new_state_a = {
                if self.state.a == 0 {
                    1
                } else {
                    0
                }
            };

            let new_state_b = {
                if self.state.b == 0 {
                    1
                } else {
                    0
                }
            };

            services.communication.send_message(2, vec![new_state_a, new_state_b]);
        }
    }

    fn save_state(&self) -> Vec<u8> {
        bincode::serialize(&self.state).unwrap()
    }

    fn load_state(&mut self, data: Vec<u8>) {
        self.state = bincode::deserialize(&data).unwrap();
    }

    fn reset_state(&mut self) {
        self.state = State { a: 0, b: 0 };
    }
}

fn main() {
    elafry::run(DemoB::new());
}
