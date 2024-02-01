mod router;
mod runner;

pub struct Agent {
    router: router::Router,
    runner: runner::Runner,
}

impl Agent {
    pub fn new() -> Agent {
        println!("Agent::new");
        Agent {
            router: router::Router::new(),
            runner: runner::Runner::new(),
        }
    }

    pub async fn demo_task1(&mut self) {
        println!("demo_task1");

        self.router.add_listener(1).await;
        let _process1 = self.runner.start("target/release/plant");

        self.router.add_listener(2).await;
        let _process2 = self.runner.start("target/release/fcs_a");

        // add required routes
        self.router
            .add_route(
                router::Address {
                    app_id: 1,
                    channel_id: 1,
                },
                router::Address {
                    app_id: 2,
                    channel_id: 1,
                },
            )
            .await;
        self.router
            .add_route(
                router::Address {
                    app_id: 2,
                    channel_id: 2,
                },
                router::Address {
                    app_id: 1,
                    channel_id: 2,
                },
            )
            .await;


    }

    pub async fn demo_task2(&mut self) {
        println!("demo_task2");

        self.router.add_listener(3).await;
        let _process3 = self.runner.start("target/release/fcs_b");

        // remove old routes
        self.router
            .remove_route(
                router::Address {
                    app_id: 1,
                    channel_id: 1,
                },
            )
            .await;
        self.router
            .remove_route(
                router::Address {
                    app_id: 2,
                    channel_id: 2,
                }
            )
            .await;

        // add new routes
        self.router
            .add_route(
                router::Address {
                    app_id: 1,
                    channel_id: 1,
                },
                router::Address {
                    app_id: 3,
                    channel_id: 1,
                },
            )
            .await;

        self.router
            .add_route(
                router::Address {
                    app_id: 3,
                    channel_id: 2,
                },
                router::Address {
                    app_id: 1,
                    channel_id: 2,
                },
            )
            .await;
    }
}
