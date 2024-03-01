use elafry::Component;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SensorData {
    position: f64,
    setpoint: f64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ControlData {
    thrust: f64,
    org_timestamp: u64,
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
}

struct Plant {
    send_message_count: u32,
    receive_message_count: u32,
    state: State,
    plant_model: PlantModel,
    writer: csv::Writer<std::fs::File>,
    loop_count: u32,
    last_timestamp: u64,
}

impl elafry::Component for Plant {
    fn new() -> Self {
        Plant {
            send_message_count: 0,
            receive_message_count: 0,
            state: State {
                thrust: 0.0,
                setpoint: 0.0,
                state_count: 0,
            },
            plant_model: PlantModel::new(),
            writer: csv::Writer::from_path("plant.csv").unwrap(),
            loop_count: 0,
            last_timestamp: 0,
        }
    }

    fn init(&mut self, _services: &mut elafry::Services) {
        eprintln!("Initializing!");
        self.send_message_count = 0;
        self.receive_message_count = 0;
        eprintln!("Starting up!");
    }

    fn run(&mut self, services: &mut elafry::Services) {
        self.loop_count += 1;

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
                    }

                    let control_data: ControlData = match bincode::deserialize(&message.data) {
                        Ok(control_data) => control_data,
                        Err(e) => {
                            println!("Failed to deserialize control_data; err = {:?}", e);
                            continue;
                        }
                    };

                    self.state.thrust = control_data.thrust;
                    self.last_timestamp = control_data.org_timestamp;
                }
                None => break,
            }
        }

        // do stuff
        self.state.state_count += 1;
        self.plant_model.update(self.state.thrust);

        // at 500, set setpoint to 20
        if self.state.state_count == 500 {
            self.state.setpoint = 20.0;
        }

        // // at 2000, set setpoint to 30
        // if self.state.state_count == 2000 {
        //     self.state.setpoint = 10.0;
        // }

        // at 3000, set setpoint to 40
        if self.state.state_count == 3000 {
            self.state.setpoint = 30.0;
        }

        // at 5000, set setpoint to 10
        if self.state.state_count == 5000 {
            self.state.setpoint = 10.0;
        }

        let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

        // form sensor data
        let sensor_data = SensorData {
            position: self.plant_model.get_position(),
            setpoint: self.state.setpoint,
        };

        self.send_message_count += 1;
        let sensor_data_buf = bincode::serialize(&sensor_data).unwrap();
        let message = elafry::communications::Message {
            channel_id: 1,
            data: sensor_data_buf,
            count: self.send_message_count,
            timestamp: timestamp,
        };
        services.communications.send_message(message);

        // calculate difference in time between now and last timestamp
        let time_diff = timestamp - self.last_timestamp;

        // write to csv
        self.writer
            .serialize((
                timestamp,
                self.last_timestamp,
                time_diff,
                sensor_data.position,
                self.state.thrust,
                self.state.setpoint,
            ))
            .unwrap();

        // kill after 6000 iterations
        if self.state.state_count == 6000 {
            self.writer.flush().unwrap();
            println!("Done!");
            std::process::exit(0);
        }
    }

    fn hello(&self) {
        eprintln!("Hello, World! (Plant)");
    }
}

fn main() {
    elafry::run(Plant::new());
}