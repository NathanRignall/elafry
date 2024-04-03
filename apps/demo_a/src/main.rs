use elafry::Component;

struct DemoA {
    state: u8,
}

impl elafry::Component for DemoA {
    fn new() -> Self {
        DemoA { state: 0 }
    }

    fn run(&mut self, services: &mut elafry::Services) {
        // do stuff with messages
        loop {
            let message = services.communication.get_message(1);
            match message {
                Some(message) => {
                    let new_state = message.data[0];

                    if new_state != self.state {
                        self.state = new_state;
                    }
                }
                None => break,
            }

            services.communication.send_message(2, vec![self.state, 1]);
        }
    }

    fn save_state(&self) -> Vec<u8> {
        bincode::serialize(&self.state).unwrap()
    }

    fn load_state(&mut self, data: Vec<u8>) {
        self.state = bincode::deserialize(&data).unwrap();
    }

    fn reset_state(&mut self) {
        self.state = 0;
    }
}

fn main() {
    elafry::run(DemoA::new());
}
