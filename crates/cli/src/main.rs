use std::fs::File;

use serde_yaml;

fn main() {
    // load configuration 1
    let configuration_1 = File::open("configuration_1.yaml").unwrap();
    let configuration: Result<elafry::types::configuration::Configuration, serde_yaml::Error> =
        serde_yaml::from_reader(configuration_1);

    // print configuration 1
    println!("{:?}", configuration);
}
