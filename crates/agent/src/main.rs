mod agent;

use std::fs::File;

use serde_yaml;

fn main() {
    // create a new agent
    let mut agent = agent::Agent::new();

    // load configuration 1
    let configuration_1 = File::open("configuration_1.yaml").unwrap();
    let configuration: Result<elafry::configuration::Configuration, serde_yaml::Error> = serde_yaml::from_reader(configuration_1);
    
    match configuration {
        Ok(configuration) => {
            for task in configuration.tasks {
                for action in task.actions {
                    agent.execute(action);
                }
            }
        },
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }

    // wait 10 seconds
    std::thread::sleep(std::time::Duration::from_secs(10));
    agent.write();

    // load configuration 2
    let configuration_2 = File::open("configuration_2.yaml").unwrap();
    let configuration: Result<elafry::configuration::Configuration, serde_yaml::Error> = serde_yaml::from_reader(configuration_2);

    match configuration {
        Ok(configuration) => {
            for task in configuration.tasks {
                for action in task.actions {
                    agent.execute(action);
                }
            }
        },
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }

    // wait 200 seconds
    std::thread::sleep(std::time::Duration::from_secs(200));
}