use elafry::Component;

struct Agent {
    loop_count: u32,
}

impl elafry::Component for Agent {
    fn new() -> Agent {
        Agent {
            loop_count: 0,
        }
    }

    fn init(&mut self, _services: &mut elafry::Services) {
        self.loop_count = 0;
    }

    fn run(&mut self, services: &mut elafry::Services) {
        self.loop_count += 1;

        // when loop_count is 5000, send a message
        if self.loop_count == 5000 {
            log::info!("-----Configuration 1-----");
            let control_data_buf = "configuration_1.yaml".as_bytes().to_vec();
            services.communication.send_message(1, control_data_buf);
        }

        // when loop_count is 25000, send a message
        if self.loop_count == 25000 {
            log::info!("-----Configuration 2-----");
            let control_data_buf = "configuration_2.yaml".as_bytes().to_vec();
            services.communication.send_message(1, control_data_buf);
        }

        // when loop_count is 45000, send a message
        if self.loop_count == 45000 {
            log::info!("-----Configuration 3-----");
            let control_data_buf = "configuration_3.yaml".as_bytes().to_vec();
            services.communication.send_message(1, control_data_buf);
        }

        // when loop_count is 50000, send a message
        if self.loop_count == 50000 {
            log::info!("-----END-----");
            let control_data_buf = "kill".as_bytes().to_vec();
            services.communication.send_message(0, control_data_buf);
        }
    }
}

fn main() {
    elafry::run(Agent::new());
}