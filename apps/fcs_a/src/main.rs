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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct PIDController {
    kp: f64, // Proportional gain
    ki: f64, // Integral gain
    kd: f64, // Derivative gain
    kn: f64, // Filter gain
    setpoint: f64,
    integral: f64,
    prev_error: f64,
}

impl PIDController {
    fn new(kp: f64, ki: f64, kd: f64, kn: f64, setpoint: f64) -> PIDController {
        PIDController {
            kp,
            ki,
            kd,
            kn,
            setpoint,
            integral: 0.0,
            prev_error: 0.0,
        }
    }

    fn set_setpoint(&mut self, setpoint: f64) {
        self.setpoint = setpoint;
    }

    fn compute(&mut self, measured_value: f64) -> f64 {
        let error = self.setpoint - measured_value;

        self.integral += error;
        let derivative = error - self.prev_error;
        self.prev_error = error;

        // use kn to filter the derivative term
        let filtered_derivative = self.kn * derivative;

        let pid_output = self.kp * error + self.ki * self.integral + self.kd * filtered_derivative;

        pid_output
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct State {
    position: f64,
    thrust: f64,
    org_timestamp: u64,
    pid_controller: PIDController,
}

struct FcsA {
    state: State,
}

impl elafry::Component for FcsA {
    fn new() -> FcsA {
        FcsA {
            state: State {
                position: 0.0,
                thrust: 0.0,
                org_timestamp: 0,
                pid_controller: PIDController::new(2.5, 0.0001,50.0, 25.0, 0.0)
            },
        }
    }

    fn run(&mut self, services: &mut elafry::Services) {
        // do stuff with messages
        loop {
            let message = services.communication.get_message(1);
            match message {
                Some(message) => {
                    let sensor_data: SensorData = match bincode::deserialize(&message.data) {
                        Ok(sensor_data) => sensor_data,
                        Err(e) => {
                            log::error!("Failed to deserialize sensor_data; err = {:?}", e);
                            continue;
                        }
                    };

                    self.state.position = sensor_data.position;
                    self.state.pid_controller.set_setpoint(sensor_data.setpoint);
                }
                None => break,
            }
        }

        // do stuff
        self.state.thrust = self.state.pid_controller.compute(self.state.position).max(0.0).min(100.0);

        // send message
        let control_data = ControlData {
            thrust: self.state.thrust,
            org_timestamp: self.state.org_timestamp,
        };
        let control_data_buf = bincode::serialize(&control_data).unwrap();
        services.communication.send_message(2, control_data_buf);
    }

    fn save_state(&self) -> Vec<u8> {
        bincode::serialize(&self.state).unwrap()
    }

    fn load_state(&mut self, data: Vec<u8>) {
        self.state = bincode::deserialize(&data).unwrap();
    }

    fn reset_state(&mut self) {
        self.state = State {
            position: 0.0,
            thrust: 0.0,
            org_timestamp: 0,
            pid_controller: PIDController::new(2.5, 0.0001,50.0, 25.0, 0.0)
        };
    }
}

fn main() {
    elafry::run(FcsA::new());
}