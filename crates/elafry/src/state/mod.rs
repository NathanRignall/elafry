pub trait Manager {
    fn set(&mut self, key: &str, value: &str);
    fn get(&self, key: &str) -> String;
}