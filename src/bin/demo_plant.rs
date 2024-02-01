use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SensorData {
    position: f64,
    setpoint: f64,
    state_count: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ControlData {
    thrust: f64,
    state_count: u32,
}

struct PlantModel {
    position: f64,
    velocity: f64,
    last_update: std::time::Instant,
}

impl PlantModel {
    fn new() -> PlantModel {
        PlantModel {
            position: 0.0,
            velocity: 0.0,
            last_update: std::time::Instant::now(),
        }
    }

    fn update(&mut self, thrust: f64) {
        // calculate dt
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;

        let gravity = 9.81;
        let mass = 1.0;
        let acceleration = thrust / mass - gravity;
        self.velocity += acceleration * dt;
        self.position += self.velocity * dt;

        // if position is below 0, set to 0 and set velocity to 0
        if self.position < 0.0 {
            self.position = 0.0;
            self.velocity = 0.0;
        }
    }

    fn get_position(&self) -> f64 {
        self.position
    }
}

struct State {
    thrust: f64,
    setpoint: f64,
    state_count: u32,
    receive_state_count: u32,
}

fn main() {
    elafry::wrapper::run(
        TestApp {
            send_message_count: 0,
            receive_message_count: 0,
            state: State {
                thrust: 0.0,
                setpoint: 0.0,
                state_count: 0,
                receive_state_count: 0,
            },
            plant_model: PlantModel::new(),
            writer: csv::Writer::from_path("plant.csv").unwrap(),
        },
        "/tmp/sock-1",
        100,
    );
}

struct TestApp {
    send_message_count: u32,
    receive_message_count: u32,
    state: State,
    plant_model: PlantModel,
    writer: csv::Writer<std::fs::File>,
}

impl elafry::wrapper::App for TestApp {
    fn init(&mut self, _services: &mut elafry::wrapper::Services) {
        self.send_message_count = 0;
        self.receive_message_count = 0;
        println!("Starting up!");
    }

    fn run(&mut self, services: &mut elafry::wrapper::Services) {
        let mut missmatch = 0;

        // do stuff with messages
        loop {
            let message = services.communications.get_message(2);
            match message {
                Some(message) => {
                    self.receive_message_count += 1;

                    if self.receive_message_count != message.count {
                        println!(
                            "--------COUNT MISMATCH-------- ({} != {})",
                            self.receive_message_count, message.count
                        );
                        self.receive_message_count = message.count;
                        missmatch = 100;
                    }

                    let control_data: ControlData = match bincode::deserialize(&message.data) {
                        Ok(control_data) => control_data,
                        Err(e) => {
                            println!("Failed to deserialize control_data; err = {:?}", e);
                            continue;
                        }
                    };

                    self.state.thrust = control_data.thrust;
                    self.state.receive_state_count = control_data.state_count;
                }
                None => break,
            }
        }

        // do stuff
        self.state.state_count += 1;
        self.plant_model.update(self.state.thrust);

        // at 1000, set setpoint to 20
        if self.state.state_count == 1000 {
            self.state.setpoint = 20.0;
        }

        // at 2000, set setpoint to 30
        if self.state.state_count == 2000 {
            self.state.setpoint = 10.0;
        }

        // at 3000, set setpoint to 40
        if self.state.state_count == 3000 {
            self.state.setpoint = 30.0;
        }

        // at 4000, set setpoint to 10
        if self.state.state_count == 4000 {
            self.state.setpoint = 10.0;
        }

        // send message
        self.send_message_count += 1;
        let sensor_data = SensorData {
            position: self.plant_model.get_position(),
            setpoint: self.state.setpoint,
            state_count: self.state.state_count,
        };
        let sensor_data_buf = bincode::serialize(&sensor_data).unwrap();
        let message = elafry::wrapper::communications::Message {
            channel_id: 1,
            data: sensor_data_buf,
            count: self.send_message_count,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
        };
        services.communications.send_message(message);

        // write to csv
        self.writer
            .serialize((
                self.state.state_count,
                self.state.receive_state_count,
                sensor_data.position,
                self.state.thrust,
                self.state.setpoint,
                missmatch,
            ))
            .unwrap();

        // kill after 1000 iterations
        if self.state.state_count == 6000 {
            self.writer.flush().unwrap();
            println!("Done!");
            std::process::exit(0);
        }
    }
}
