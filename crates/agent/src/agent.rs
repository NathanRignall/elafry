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
                self.runner.add_component(action.data.app_id, &action.data.component);
            },
            elafry::configuration::Action::StartComponent(action) => {
                self.runner.start_component(action.data.app_id);
            },
            elafry::configuration::Action::StopComponent(action) => {
                self.runner.stop_component(action.data.app_id);
            },
            elafry::configuration::Action::RemoveComponent(action) => {
                self.runner.remove_component(action.data.app_id);
            },
            elafry::configuration::Action::AddRoute(action) => {
                self.runner.add_route(action.data.source, action.data.destination);
            },
            elafry::configuration::Action::RemoveRoute(action) => {
                self.runner.remove_route(action.data.source);
            },
        }
    }

    pub fn write(&mut self) {
        self.runner.write();
    }
}
