mod runner;

pub struct Agent {
    runner: runner::Runner,

    process1: uuid::Uuid,
    process2: uuid::Uuid,
    process3: uuid::Uuid,
}

impl Agent {
    pub fn new() -> Agent {
        println!("Agent::new");
        let mut runner = runner::Runner::new();

        runner.run();

        Agent { runner, process1: uuid::Uuid::new_v4(), process2: uuid::Uuid::new_v4(), process3: uuid::Uuid::new_v4() }
    }

    pub fn demo_task1(&mut self) {
        println!("demo_task1");

        // add required routes
        self.runner.add_route(
            runner::Address {
                app_id: self.process1,
                channel_id: 1,
            },
            runner::Address {
                app_id: self.process2,
                channel_id: 1,
            },
        );
        self.runner.add_route(
            runner::Address {
                app_id: self.process2,
                channel_id: 2,
            },
            runner::Address {
                app_id: self.process1,
                channel_id: 2,
            },
        );

        // add processes
        self.runner.add(self.process1, "plant");
        self.runner.add(self.process2, "fcs_a");

        // start processes
        self.runner.start(self.process1);
        self.runner.start(self.process2);
    }

    pub fn demo_task2(&mut self) {
        println!("demo_task2");

        // write instrument
        self.runner.write();

        // stop processes
        self.runner.stop(self.process1);
        self.runner.stop(self.process2);

        // remove processes
        self.runner.remove(self.process1);
        self.runner.remove(self.process2);

        // // add processes
        // self.runner.add(self.process3, "fcs_b");

        // // stop old processes
        // self.runner.stop(self.process2);

        // // remove old routes
        // self.runner.remove_route(runner::Address {
        //     app_id: self.process1,
        //     channel_id: 1,
        // });
        // self.runner.remove_route(runner::Address {
        //     app_id: self.process2,
        //     channel_id: 2,
        // });

        // // add new routes
        // self.runner.add_route(
        //     runner::Address {
        //         app_id: self.process1,
        //         channel_id: 1,
        //     },
        //     runner::Address {
        //         app_id: self.process3,
        //         channel_id: 1,
        //     },
        // );
        // self.runner.add_route(
        //     runner::Address {
        //         app_id: self.process3,
        //         channel_id: 2,
        //     },
        //     runner::Address {
        //         app_id: self.process1,
        //         channel_id: 2,
        //     },
        // );

        // // start processes
        // self.runner.start(self.process3);

        // // remove old processes
        // self.runner.remove(self.process2);
    }
}
