pub trait Worker {
    fn create(&mut self);
    fn kill(&mut self);
}
