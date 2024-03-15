mod runner;

pub struct Agent {
    runner: runner::Runner,
}

impl Agent {
    pub fn new() -> Agent {
        println!("Agent::new");
        let mut runner = runner::Runner::new();

        runner.run();

        Agent { runner }
    }

    pub fn execute(&mut self, action: elafry::configuration::Action) {
        println!("Agent::execute");
        match action {
            elafry::configuration::Action::AddComponent(action) => {
                self.runner
                    .add_component(action.data.component_id, &action.data.component);
            }
            elafry::configuration::Action::StartComponent(action) => {
                self.runner.start_component(action.data.component_id);
            }
            elafry::configuration::Action::StopComponent(action) => {
                self.runner.stop_component(action.data.component_id);
            }
            elafry::configuration::Action::RemoveComponent(action) => {
                self.runner.remove_component(action.data.component_id);
            }
            elafry::configuration::Action::AddRoute(action) => {
                self.runner.add_route(
                    runner::RouteEndpoint {
                        component_id: action.data.source.component_id,
                        channel_id: action.data.source.channel_id,
                    },
                    runner::RouteEndpoint {
                        component_id: action.data.target.component_id,
                        channel_id: action.data.target.channel_id,
                    },
                );
            }
            elafry::configuration::Action::RemoveRoute(action) => {
                self.runner.remove_route(runner::RouteEndpoint {
                    component_id: action.data.source.component_id,
                    channel_id: action.data.source.channel_id,
                });
            }
            elafry::configuration::Action::SetSchedule(action) => {
                self.runner.set_schedule(runner::Schedule {
                    period: std::time::Duration::from_micros(1_000_000 / action.data.frequency),
                    major_frames: action
                        .data
                        .major_frames
                        .into_iter()
                        .map(|frame| runner::MajorFrame {
                            minor_frames: frame
                                .minor_frames
                                .into_iter()
                                .map(|frame| runner::MinorFrame {
                                    component_id: frame.component_id,
                                })
                                .collect(),
                        })
                        .collect(),
                });
            }
        }
    }

    pub fn write(&mut self) {
        self.runner.write();
    }
}
