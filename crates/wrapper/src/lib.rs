pub mod communications;

pub trait App {
    fn init(&mut self, services: &mut Services);
    fn run(&mut self, services: &mut Services);
}

pub struct Services {
    pub communications: communications::Manager,
}

pub struct Runner {
    app: Box<dyn App>,
    running: bool,
    services: Services,
}

impl Runner {
    pub fn create<T: App + 'static>(app: T, path: &str) -> Runner {
        Runner {
            app: Box::new(app),
            running: false,
            services: Services {
                communications: communications::Manager::new(path),
            },
        }
    }

    pub fn start(&mut self) {
        self.running = true;
        self.app.init(&mut self.services);
    }

    pub fn stop(&mut self) {}

    pub fn delete(&self) {
        println!("Hello, World!");
    }

    pub fn run(&mut self) {
        self.services.communications.receive();

        self.app.run(&mut self.services);
    }
}

pub fn run<T: App + 'static>(app: T, path: &str, frequency: u32) {
    let mut runner = Runner::create(app, path);

    let mut last_time;
    let period = std::time::Duration::from_micros(1_000_000 / frequency as u64);

    runner.start();

    while runner.running {
        last_time = std::time::Instant::now();

        runner.run();

        let now = std::time::Instant::now();
        let duration = now.duration_since(last_time);

        if duration < period {
            std::thread::sleep(period - duration);
        } else {
            println!("Warning: loop took longer than period {}ms", period.as_millis());
        }
    }

    runner.stop();
    runner.delete();
}
