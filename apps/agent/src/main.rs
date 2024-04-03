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

    fn run(&mut self, services: &mut elafry::Services) {
        self.loop_count += 1;

        // when loop_count is 5000, send a message
        if self.loop_count == 5 {
            log::info!("-----Plant Configuration 1-----");
            let control_data_buf = "plant_1.yaml".as_bytes().to_vec();
            services.communication.send_message(1, control_data_buf);
        }

        // when loop_count is 25000, send a message
        if self.loop_count == 25000 {
            log::info!("-----Plant Configuration 2-----");
            let control_data_buf = "plant_2.yaml".as_bytes().to_vec();
            services.communication.send_message(1, control_data_buf);
        }

        // when loop_count is 45000, send a message
        if self.loop_count == 45000 {
            log::info!("-----Plant Configuration 3-----");
            let control_data_buf = "plant_3.yaml".as_bytes().to_vec();
            services.communication.send_message(1, control_data_buf);
        }

        // when loop_count is 50000, send a message
        if self.loop_count == 50000 {
            log::info!("-----END-----");
            let control_data_buf = "kill".as_bytes().to_vec();
            services.communication.send_message(0, control_data_buf);
        }

        // // when loop_count is 5, send a message
        // if self.loop_count == 5 {
        //     log::info!("-----Demo Configuration 1-----");
        //     let control_data_buf = "demo_1.yaml".as_bytes().to_vec();
        //     services.communication.send_message(1, control_data_buf);
        // }
    }

    fn save_state(&self) -> Vec<u8> {
        bincode::serialize(&self.loop_count).unwrap()
    }

    fn load_state(&mut self, data: Vec<u8>) {
        self.loop_count = bincode::deserialize(&data).unwrap();
    }

    fn reset_state(&mut self) {
        self.loop_count = 0;
    }
}

fn main() {
    elafry::run(Agent::new());
}