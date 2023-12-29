pub trait InterruptHandler {
    fn trigger(&mut self);
}