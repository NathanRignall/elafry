use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Configuration {
    pub tasks: Vec<Task>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Task {
    pub id: uuid::Uuid,
    pub actions: Action,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Action {
    #[serde(rename = "blocking")]
    Blocking(Vec<BlockingAction>),
    #[serde(rename = "non-blocking")]
    NonBlocking(Vec<NonBlockingAction>),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct BlockingAction {
    pub id: uuid::Uuid,
    pub data: BlockingData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct NonBlockingAction {
    pub id: uuid::Uuid,
    pub data: NonBlockingData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum BlockingData {
    #[serde(rename = "start-component")]
    StartComponent(StartComponentData),
    #[serde(rename = "stop-component")]
    StopComponent(StopComponentData),
    #[serde(rename = "add-route")]
    AddRoute(AddRouteData),
    #[serde(rename = "remove-route")]
    RemoveRoute(RemoveRouteData),
    #[serde(rename = "set-schedule")]
    SetSchedule(SetScheduleData),
    #[serde(rename = "add-state-sync")]
    AddStateSync(AddStateSyncData),
    #[serde(rename = "remove-state-sync")]
    RemoveStateSync(RemoveStateSyncData),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum NonBlockingData {
    #[serde(rename = "add-component")]
    AddComponent(AddComponentData),
    #[serde(rename = "remove-component")]
    RemoveComponent(RemoveComponentData),
    #[serde(rename = "wait-state-sync")]
    WaitStateSync(WaitStateSyncData),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct AddComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
    pub component: String,
    pub core: usize,
    pub version: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct StartComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct StopComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct RemoveComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct AddRouteData {
    pub source: RouteEndpoint,
    pub target: RouteEndpoint,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Eq, Hash, Clone)]
pub struct RemoveRouteData {
    pub source: RouteEndpoint,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Eq, Hash, Clone)]
pub struct RouteEndpoint {
    pub endpoint: Endpoint,
    #[serde(rename = "channel-id")]
    pub channel_id: u32,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Eq, Hash, Clone)]
pub enum Endpoint {
    #[serde(rename = "component-id")]
    Component(uuid::Uuid),
    #[serde(rename = "address")]
    Address(String),
    #[serde(rename = "runner")]
    Runner,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SetScheduleData {
    pub deadline: u64,
    #[serde(rename = "major-frames")]
    pub major_frames: Vec<MajorFrame>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MajorFrame {
    #[serde(rename = "minor-frames")]
    pub minor_frames: Vec<MinorFrame>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MinorFrame {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
    pub deadline: u64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct AddStateSyncData {
    #[serde(rename = "state-sync-id")]
    pub state_sync_id: uuid::Uuid,
    pub source: StateEndpoint,
    pub target: StateEndpoint,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct RemoveStateSyncData {
    #[serde(rename = "state-sync-id")]
    pub state_sync_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct WaitStateSyncData {
    #[serde(rename = "state-sync-id")]
    pub state_sync_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct StateEndpoint {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;

    // setup logging
    fn setup() {
        let _ = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("warn,info,debug,trace"),
        )
        .is_test(true)
        .try_init();
    }

    #[test]
    fn test_configuration() {
        setup();

        let configuration = Configuration {
            tasks: vec![

                Task {
                    id: uuid::Uuid::new_v4(),
                    actions: Action::Blocking(vec![
                        BlockingAction {
                            id: uuid::Uuid::new_v4(),
                            data: BlockingData::StartComponent(StartComponentData {
                                component_id: uuid::Uuid::new_v4(),
                            }),
                        },
                    ]),
                },

                Task {
                    id: uuid::Uuid::new_v4(),
                    actions: Action::Blocking(vec![
                        BlockingAction {
                            id: uuid::Uuid::new_v4(),
                            data: BlockingData::StopComponent(StopComponentData {
                                component_id: uuid::Uuid::new_v4(),
                            }),
                        },
                    ]),
                },

                Task {
                    id: uuid::Uuid::new_v4(),
                    actions: Action::Blocking(vec![
                        BlockingAction {
                            id: uuid::Uuid::new_v4(),
                            data: BlockingData::AddRoute(AddRouteData {
                                source: RouteEndpoint {
                                    endpoint: Endpoint::Component(uuid::Uuid::new_v4()),
                                    channel_id: 1,
                                },
                                target: RouteEndpoint {
                                    endpoint: Endpoint::Component(uuid::Uuid::new_v4()),
                                    channel_id: 2,
                                },
                            }),
                        },
                    ]),
                },
                
                Task {
                    id: uuid::Uuid::new_v4(),
                    actions: Action::Blocking(vec![
                        BlockingAction {
                            id: uuid::Uuid::new_v4(),
                            data: BlockingData::RemoveRoute(RemoveRouteData {
                                source: RouteEndpoint {
                                    endpoint: Endpoint::Component(uuid::Uuid::new_v4()),
                                    channel_id: 1,
                                },
                            }),
                        },
                    ]),
                },

                Task {
                    id: uuid::Uuid::new_v4(),
                    actions: Action::Blocking(vec![
                        BlockingAction {
                            id: uuid::Uuid::new_v4(),
                            data: BlockingData::SetSchedule(SetScheduleData {
                                deadline: 1,
                                major_frames: vec![
                                    MajorFrame {
                                        minor_frames: vec![
                                            MinorFrame {
                                                component_id: uuid::Uuid::new_v4(),
                                                deadline: 2,
                                            },
                                        ],
                                    },
                                ],
                            }),
                        },
                    ]),
                },

                Task {
                    id: uuid::Uuid::new_v4(),
                    actions: Action::Blocking(vec![
                        BlockingAction {
                            id: uuid::Uuid::new_v4(),
                            data: BlockingData::AddStateSync(AddStateSyncData {
                                state_sync_id: uuid::Uuid::new_v4(),
                                source: StateEndpoint {
                                    component_id: uuid::Uuid::new_v4(),
                                },
                                target: StateEndpoint {
                                    component_id: uuid::Uuid::new_v4(),
                                },
                            }),
                        },
                    ]),
                },

                Task {
                    id: uuid::Uuid::new_v4(),
                    actions: Action::Blocking(vec![
                        BlockingAction {
                            id: uuid::Uuid::new_v4(),
                            data: BlockingData::RemoveStateSync(RemoveStateSyncData {
                                state_sync_id: uuid::Uuid::new_v4(),
                            }),
                        },
                    ]),
                },

                Task {
                    id: uuid::Uuid::new_v4(),
                    actions: Action::NonBlocking(vec![
                        NonBlockingAction {
                            id: uuid::Uuid::new_v4(),
                            data: NonBlockingData::AddComponent(AddComponentData {
                                component_id: uuid::Uuid::new_v4(),
                                component: "component".to_string(),
                                core: 1,
                                version: "version".to_string(),
                            }),
                        },
                    ]),
                },

                Task {
                    id: uuid::Uuid::new_v4(),
                    actions: Action::NonBlocking(vec![
                        NonBlockingAction {
                            id: uuid::Uuid::new_v4(),
                            data: NonBlockingData::RemoveComponent(RemoveComponentData {
                                component_id: uuid::Uuid::new_v4(),
                            }),
                        },
                    ]),
                },

                Task {
                    id: uuid::Uuid::new_v4(),
                    actions: Action::NonBlocking(vec![
                        NonBlockingAction {
                            id: uuid::Uuid::new_v4(),
                            data: NonBlockingData::WaitStateSync(WaitStateSyncData {
                                state_sync_id: uuid::Uuid::new_v4(),
                            }),
                        },
                    ]),
                },
                
            
            ],
        };

        // test using yaml
        let serialized = serde_yaml::to_string(&configuration).unwrap();
        let deserialized: Configuration = serde_yaml::from_str(&serialized).unwrap();

        assert_eq!(configuration, deserialized);
    }

    #[test]
    fn test_types_debug() {
        setup();
        
        let uuid = uuid::Uuid::new_v4();

        let configuration = Configuration {
            tasks: vec![]
        };
        let serialized = format!("{:?}", configuration);
        assert_eq!(serialized, "Configuration { tasks: [] }");

        let task = Task {
            id: uuid,
            actions: Action::Blocking(vec![])
        };
        let serialized = format!("{:?}", task);
        let expected = format!("Task {{ id: {}, actions: Blocking([]) }}", uuid);
        assert_eq!(serialized, expected);

        let action = Action::Blocking(vec![]);
        let serialized = format!("{:?}", action);
        let expected = format!("Blocking([])");
        assert_eq!(serialized, expected);

        let blocking_action = BlockingAction {
            id: uuid,
            data: BlockingData::StartComponent(StartComponentData {
                component_id: uuid
            })
        };
        let serialized = format!("{:?}", blocking_action);
        let expected = format!("BlockingAction {{ id: {}, data: StartComponent(StartComponentData {{ component_id: {} }}) }}", uuid, uuid);
        assert_eq!(serialized, expected);

        let non_blocking_action = NonBlockingAction {
            id: uuid,
            data: NonBlockingData::AddComponent(AddComponentData {
                component_id: uuid,
                component: "component".to_string(),
                core: 1,
                version: "version".to_string()
            })
        };
        let serialized = format!("{:?}", non_blocking_action);
        let expected = format!("NonBlockingAction {{ id: {}, data: AddComponent(AddComponentData {{ component_id: {}, component: \"component\", core: 1, version: \"version\" }}) }}", uuid, uuid);
        assert_eq!(serialized, expected);

        let blocking_data = BlockingData::StartComponent(StartComponentData {
            component_id: uuid
        });
        let serialized = format!("{:?}", blocking_data);
        let expected = format!("StartComponent(StartComponentData {{ component_id: {} }})", uuid);
        assert_eq!(serialized, expected);

        let non_blocking_data = NonBlockingData::AddComponent(AddComponentData {
            component_id: uuid,
            component: "component".to_string(),
            core: 1,
            version: "version".to_string()
        });
        let serialized = format!("{:?}", non_blocking_data);
        let expected = format!("AddComponent(AddComponentData {{ component_id: {}, component: \"component\", core: 1, version: \"version\" }})", uuid);
        assert_eq!(serialized, expected);

        let blocking_data = BlockingData::StartComponent(StartComponentData {
            component_id: uuid
        });
        let serialized = format!("{:?}", blocking_data);
        let expected = format!("StartComponent(StartComponentData {{ component_id: {} }})", uuid);
        assert_eq!(serialized, expected);

        let non_blocking_data = NonBlockingData::AddComponent(AddComponentData {
            component_id: uuid,
            component: "component".to_string(),
            core: 1,
            version: "version".to_string()
        });
        let serialized = format!("{:?}", non_blocking_data);
        let expected = format!("AddComponent(AddComponentData {{ component_id: {}, component: \"component\", core: 1, version: \"version\" }})", uuid);
        assert_eq!(serialized, expected);

        let remove_component_data = RemoveComponentData {
            component_id: uuid
        };
        let serialized = format!("{:?}", remove_component_data);
        let expected = format!("RemoveComponentData {{ component_id: {} }}", uuid);
        assert_eq!(serialized, expected);

        let add_route_data = AddRouteData {
            source: RouteEndpoint {
                endpoint: Endpoint::Component(uuid),
                channel_id: 1
            },
            target: RouteEndpoint {
                endpoint: Endpoint::Component(uuid),
                channel_id: 2
            }
        };
        let serialized = format!("{:?}", add_route_data);
        let expected = format!("AddRouteData {{ source: RouteEndpoint {{ endpoint: Component({}), channel_id: 1 }}, target: RouteEndpoint {{ endpoint: Component({}), channel_id: 2 }} }}", uuid, uuid);
        assert_eq!(serialized, expected);
        
        let remove_route_data = RemoveRouteData {
            source: RouteEndpoint {
                endpoint: Endpoint::Component(uuid),
                channel_id: 1
            }
        };
        let serialized = format!("{:?}", remove_route_data);
        let expected = format!("RemoveRouteData {{ source: RouteEndpoint {{ endpoint: Component({}), channel_id: 1 }} }}", uuid);
        assert_eq!(serialized, expected);
    }
}