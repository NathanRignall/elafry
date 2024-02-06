mod router;
mod runner;

pub struct Agent {
    router: router::Router,
    runner: runner::Runner,
}

impl Agent {
    pub fn new() -> Agent {
        println!("Agent::new");
        let mut router = router::Router::new();
        let runner = runner::Runner::new();

        router.start();

        Agent { router, runner }
    }

    pub fn demo_task1(&mut self) {
        println!("demo_task1");

        // add required listeners
        self.router.add_listener(1);
        self.router.add_listener(2);

        // add required routes
        self.router.add_route(
            router::Address {
                app_id: 1,
                channel_id: 1,
            },
            router::Address {
                app_id: 2,
                channel_id: 1,
            },
        );
        self.router.add_route(
            router::Address {
                app_id: 2,
                channel_id: 2,
            },
            router::Address {
                app_id: 1,
                channel_id: 2,
            },
        );

        // start processes
        let _process1 = self.runner.start("target/release/plant");
        let _process2 = self.runner.start("target/release/fcs_a");
    }

    pub fn demo_task2(&mut self) {
        println!("demo_task2");

        // add required listeners
        self.router.add_listener(3);

        // start processes
        let _process3 = self.runner.start("target/release/fcs_b");

        // remove old routes
        self.router.remove_route(router::Address {
            app_id: 1,
            channel_id: 1,
        });
        self.router.remove_route(router::Address {
            app_id: 2,
            channel_id: 2,
        });

        // add new routes
        self.router.add_route(
            router::Address {
                app_id: 1,
                channel_id: 1,
            },
            router::Address {
                app_id: 3,
                channel_id: 1,
            },
        );

        self.router.add_route(
            router::Address {
                app_id: 3,
                channel_id: 2,
            },
            router::Address {
                app_id: 1,
                channel_id: 2,
            },
        );
    }
}
