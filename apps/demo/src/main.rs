use elafry::{services::communication, Component};

struct State {
    thrust: f64,
    setpoint: f64,
    state_count: u32,
}

struct Demo {
    send_message_count: u32,
    receive_message_count: u32,
    loop_count: u32,
    last_timestamp: u64,
    state: u8,
    state_count: u32,
}

impl elafry::Component for Demo {
    fn new() -> Self {
        Demo {
            send_message_count: 0,
            receive_message_count: 0,
            loop_count: 0,
            last_timestamp: 0,
            state: 0,
            state_count: 0,
        }
    }

    fn init(&mut self, _services: &mut elafry::Services) {
        self.send_message_count = 0;
        self.receive_message_count = 0;
    }

    fn run(&mut self, services: &mut elafry::Services) {
        self.loop_count += 1;

        // do stuff with messages
        loop {
            let message = services.communication.get_message(2);
            match message {
                Some(message) => {
                    self.receive_message_count += 1;

                    let new_state = message.data[0];

                    if new_state != self.state {
                        println!("State changed from {} to {}", self.state, new_state);
                    }
                }
                None => break,
            }
        }

        // services.communication.send_message(2, vec![69]);
        
    }
}

fn main() {
    elafry::run(Demo::new());
}