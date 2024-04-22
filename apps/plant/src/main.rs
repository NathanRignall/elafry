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
    timestamp: u64,
    loop_count: u64,
    update: bool,
}

struct PlantModel {
    last_update: std::time::Instant,
}

impl PlantModel {
    fn new() -> PlantModel {
        PlantModel {
            last_update: std::time::Instant::now(),
        }
    }

    fn update(&mut self, position: &mut f64, velocity: &mut f64, thrust: f64) {
        // calculate dt
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;

        let gravity = 9.81;
        let mass = 1.0;
        let acceleration = thrust / mass - gravity;
        *velocity += acceleration * dt;
        *position += *velocity * dt;

        // if position is below 0, set to 0 and set velocity to 0
        if *position < 0.0 {
            *position = 0.0;
            *velocity = 0.0;
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct State {
    thrust: f64,
    setpoint: f64,
    state_count: u32,
    position: f64,
    velocity: f64,
}

struct Plant {
    writer: csv::Writer<std::fs::File>,
    last_timestamp: u64,
    last_loop_count: u64,
    last_update: bool,
    state: State,
    plant_model: PlantModel,
}

impl elafry::Component for Plant {
    fn new() -> Self {
        Plant {
            state: State {
                thrust: 0.0,
                setpoint: 0.0,
                state_count: 0,
                position: 0.0,
                velocity: 0.0,
            },
            plant_model: PlantModel::new(),
            writer: csv::Writer::from_path("plant.csv").unwrap(),
            last_timestamp: 0,
            last_loop_count: 0,
            last_update: false,
        }
    }

    fn run(&mut self, services: &mut elafry::Services) {
        // reset last loop count
        self.last_loop_count = 0;

        // do stuff with messages
        loop {
            let message = services.communication.get_message(2);
            match message {
                Some(message) => {
                    let control_data: ControlData = match bincode::deserialize(&message.data) {
                        Ok(control_data) => control_data,
                        Err(e) => {
                            log::error!("Failed to deserialize control_data; err = {:?}", e);
                            continue;
                        }
                    };

                    self.state.thrust = control_data.thrust;
                    self.last_timestamp = control_data.timestamp;
                    self.last_loop_count = control_data.loop_count;
                    self.last_update = control_data.update;
                }
                None => break,
            }
        }

        // do stuff
        self.state.state_count += 1;
        self.plant_model.update(&mut self.state.position, &mut self.state.velocity, self.state.thrust);

        // at 200, set setpoint to 50
        if self.state.state_count == 200 {
            self.state.setpoint = 10.0;
        }

        // at 5000, set setpoint to 20
        if self.state.state_count == 5000 {
            self.state.setpoint = 20.0;
        }

        // // at 3000, set setpoint to 40
        // if self.state.state_count == 8000 {
        //     self.state.setpoint = 100.0;
        // }

        // // at 5000, set setpoint to 10
        // if self.state.state_count == 5000 {
        //     self.state.setpoint = 10.0;
        // }

        let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

        // form sensor data
        let sensor_data = SensorData {
            position: self.state.position,
            setpoint: self.state.setpoint,
        };

        let sensor_data_buf = bincode::serialize(&sensor_data).unwrap();
        services.communication.send_message(1, sensor_data_buf);

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
                self.last_loop_count,
                self.last_update,
            ))
            .unwrap();
    }

    fn save_state(&self) -> Vec<u8> {
        bincode::serialize(&self.state).unwrap()
    }

    fn load_state(&mut self, data: Vec<u8>) {
        // try to deserialize the data and print an error if it fails
        match bincode::deserialize(&data) {
            Ok(state) => self.state = state,
            Err(e) => log::error!("Failed to deserialize state; err = {:?}", e),
        }
    }

    fn reset_state(&mut self) {
        self.state = State {
            thrust: 0.0,
            setpoint: 0.0,
            state_count: 0,
            position: 0.0,
            velocity: 0.0,
        };
    }

}

fn main() {
    elafry::run(Plant::new());
}