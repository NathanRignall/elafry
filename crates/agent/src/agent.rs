mod router;
mod runner;

pub struct Agent {
    router: router::Router,
    runner: runner::Runner,

    process1: uuid::Uuid,
    process2: uuid::Uuid,
    process3: uuid::Uuid,
}

impl Agent {
    pub fn new() -> Agent {
        println!("Agent::new");
        let mut router = router::Router::new();
        let mut runner = runner::Runner::new();

        runner.run();   
        router.run();

        Agent { router, runner, process1: uuid::Uuid::new_v4(), process2: uuid::Uuid::new_v4(), process3: uuid::Uuid::new_v4() }
    }

    pub fn demo_task1(&mut self) {
        println!("demo_task1");

        // add required listeners
        self.router.add_listener(self.process1);
        self.router.add_listener(self.process2);

        // add required routes
        self.router.add_route(
            router::Address {
                app_id: self.process1,
                channel_id: 1,
            },
            router::Address {
                app_id: self.process2,
                channel_id: 1,
            },
        );
        self.router.add_route(
            router::Address {
                app_id: self.process2,
                channel_id: 2,
            },
            router::Address {
                app_id: self.process1,
                channel_id: 2,
            },
        );

        // add processes
        self.runner.add(self.process1, "libplant.dylib");
        self.runner.add(self.process2, "libfcs_a.dylib");

        // start processes
        self.runner.start(self.process1);
        self.runner.start(self.process2);
    }

    pub fn demo_task2(&mut self) {
        println!("demo_task2");

        // add required listeners
        self.router.add_listener(self.process3);

        // add processes
        self.runner.add(self.process3, "libfcs_b.dylib");

        // stop old processes
        self.runner.stop(self.process2);

        // remove old routes
        self.router.remove_route(router::Address {
            app_id: self.process1,
            channel_id: 1,
        });
        self.router.remove_route(router::Address {
            app_id: self.process2,
            channel_id: 2,
        });

        // add new routes
        self.router.add_route(
            router::Address {
                app_id: self.process1,
                channel_id: 1,
            },
            router::Address {
                app_id: self.process3,
                channel_id: 1,
            },
        );
        self.router.add_route(
            router::Address {
                app_id: self.process3,
                channel_id: 2,
            },
            router::Address {
                app_id: self.process1,
                channel_id: 2,
            },
        );

        // start processes
        self.runner.start(self.process3);

        // remove old processes
        self.runner.remove(self.process2);
    }
}
