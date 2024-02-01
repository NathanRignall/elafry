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

struct PIDController {
    kp: f64, // Proportional gain
    ki: f64, // Integral gain
    kd: f64, // Derivative gain
    setpoint: f64,
    integral: f64,
    prev_error: f64,
}

impl PIDController {
    fn new(kp: f64, ki: f64, kd: f64, setpoint: f64) -> PIDController {
        PIDController {
            kp,
            ki,
            kd,
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

        // Proportional term
        let p_term = self.kp * error;

        // Integral term
        self.integral += error;
        let i_term = self.ki * self.integral;

        // Derivative term
        let d_term = self.kd * (error - self.prev_error);
        self.prev_error = error;

        // Calculate PID output
        let pid_output = p_term + i_term + d_term;

        pid_output
    }
}

struct State {
    position: f64,
    thrust: f64,
    state_count: u32,
}

fn main() {
    wrapper::run(
        TestApp {
            send_message_count: 0,
            receive_message_count: 0,
            state: State {
                position: 0.0,
                thrust: 0.0,
                state_count: 0,
            },
            pid_controller: PIDController::new(1.0, 0.005, 0.5, 0.0),
        },
        "/tmp/sock-2",
        100,
    );
}

struct TestApp {
    send_message_count: u32,
    receive_message_count: u32,
    state: State,
    pid_controller: PIDController,
}

impl wrapper::App for TestApp {
    fn init(&mut self, _services: &mut wrapper::Services) {
        self.receive_message_count = 0;
        self.send_message_count = 0;
        println!("Starting up!");
    }

    fn run(&mut self, services: &mut wrapper::Services) {
        // do stuff with messages
        loop {
            let message = services.communications.get_message(1);
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

                    let sensor_data: SensorData = match bincode::deserialize(&message.data) {
                        Ok(sensor_data) => sensor_data,
                        Err(e) => {
                            println!("Failed to deserialize sensor_data; err = {:?}", e);
                            continue;
                        }
                    };

                    self.state.position = sensor_data.position;
                    self.pid_controller.set_setpoint(sensor_data.setpoint);
                    self.state.state_count = sensor_data.state_count;
                }
                None => break,
            }
        }

        // do stuff
        self.state.thrust = self.pid_controller.compute(self.state.position).max(0.0).min(100.0);

        // send message
        self.send_message_count += 1;
        let control_data = ControlData {
            thrust: self.state.thrust,
            state_count: self.state.state_count,
        };
        let control_data_buf = bincode::serialize(&control_data).unwrap();
        let message = wrapper::communications::Message {
            channel_id: 2,
            data: control_data_buf,
            count: self.send_message_count,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
        };
        services.communications.send_message(message);
    }
}
