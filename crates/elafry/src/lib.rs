pub mod communications;
pub mod messages;
pub mod state;

pub trait Component {
    fn init(&mut self, services: &mut Services);
    fn run(&mut self, services: &mut Services);
    fn hello(&self);
}

pub struct Services {
    pub communications: communications::Manager,
}