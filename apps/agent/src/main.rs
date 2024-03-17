use elafry::Component;

struct Agent {
    loop_count: u32,
    send_message_count: u32,
}

impl elafry::Component for Agent {
    fn new() -> Agent {
        Agent {
            loop_count: 0,
            send_message_count: 0,
        }
    }

    fn init(&mut self, _services: &mut elafry::Services) {
        self.loop_count = 0;
        println!("Starting up! XXXXX");
    }

    fn run(&mut self, services: &mut elafry::Services) {
        self.loop_count += 1;

        // when loop_count is 1000, send a message
        if self.loop_count == 500 {
            println!("Configuration 1");
            let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

            let control_data_buf = "configuration_1.yaml".as_bytes().to_vec();
            let message = elafry::types::communication::Message {
                channel_id: 1,
                data: control_data_buf,
                count: self.send_message_count,
                timestamp: timestamp,
            };
            services.communication.send_message(message);
        }

        // when loop_count is 6000, send a message
        if self.loop_count == 2000 {
            println!("Configuration 2");
            let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

            let control_data_buf = "configuration_2.yaml".as_bytes().to_vec();
            let message = elafry::types::communication::Message {
                channel_id: 1,
                data: control_data_buf,
                count: self.send_message_count,
                timestamp: timestamp,
            };
            services.communication.send_message(message);
        }

        // when loop_count is 11000, send a message
        if self.loop_count == 3000 {
            println!("Configuration 3");
            let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

            let control_data_buf = "configuration_3.yaml".as_bytes().to_vec();
            let message = elafry::types::communication::Message {
                channel_id: 1,
                data: control_data_buf,
                count: self.send_message_count,
                timestamp: timestamp,
            };
            services.communication.send_message(message);
        }
    }
}

fn main() {
    elafry::run(Agent::new());
}