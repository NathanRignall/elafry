use std::fs::File;

use serde_yaml;

fn main() {
    // load configuration 1
    let configuration_1 = File::open("configuration_1.yaml").unwrap();
    let configuration: Result<elafry::types::configuration::Configuration, serde_yaml::Error> =
        serde_yaml::from_reader(configuration_1);

    // print configuration 1
    println!("{:?}", configuration);

    // create an example configuration
    let configuration = elafry::types::configuration::Configuration {
        tasks: vec![elafry::types::configuration::Task {
            id: uuid::Uuid::new_v4(),
            actions: vec![
                elafry::types::configuration::Action::AddComponent(
                    elafry::types::configuration::AddComponentAction {
                        id: uuid::Uuid::new_v4(),
                        data: elafry::types::configuration::AddComponentData {
                            component_id: uuid::Uuid::new_v4(),
                            component: "component".to_string(),
                            core: 0,
                            version: "0.0.0".to_string(),
                        },
                    },
                ),
                elafry::types::configuration::Action::StartComponent(
                    elafry::types::configuration::StartComponentAction {
                        id: uuid::Uuid::new_v4(),
                        data: elafry::types::configuration::StartComponentData {
                            component_id: uuid::Uuid::new_v4(),
                        },
                    },
                ),
                elafry::types::configuration::Action::StopComponent(
                    elafry::types::configuration::StopComponentAction {
                        id: uuid::Uuid::new_v4(),
                        data: elafry::types::configuration::StopComponentData {
                            component_id: uuid::Uuid::new_v4(),
                        },
                    },
                ),
                elafry::types::configuration::Action::RemoveComponent(
                    elafry::types::configuration::RemoveComponentAction {
                        id: uuid::Uuid::new_v4(),
                        data: elafry::types::configuration::RemoveComponentData {
                            component_id: uuid::Uuid::new_v4(),
                        },
                    },
                ),
                elafry::types::configuration::Action::AddRoute(
                    elafry::types::configuration::AddRouteAction {
                        id: uuid::Uuid::new_v4(),
                        data: elafry::types::configuration::AddRouteData {
                            source: elafry::types::configuration::RouteEndpoint {
                                endpoint: elafry::types::configuration::Endpoint::Component(
                                    uuid::Uuid::new_v4(),
                                ),
                                channel_id: 0,
                            },
                            target: elafry::types::configuration::RouteEndpoint {
                                endpoint: elafry::types::configuration::Endpoint::Component(
                                    uuid::Uuid::new_v4(),
                                ),
                                channel_id: 0,
                            },
                        },
                    },
                ),
                elafry::types::configuration::Action::RemoveRoute(
                    elafry::types::configuration::RemoveRouteAction {
                        id: uuid::Uuid::new_v4(),
                        data: elafry::types::configuration::RemoveRouteData {
                            source: elafry::types::configuration::RouteEndpoint {
                                endpoint: elafry::types::configuration::Endpoint::Component(
                                    uuid::Uuid::new_v4(),
                                ),
                                channel_id: 0,
                            },
                        },
                    },
                ),
            ],
        }],
    };

    // convert the configuration to a string
    let configuration_string = serde_yaml::to_string(&configuration).unwrap();
    println!("{}", configuration_string);
}
