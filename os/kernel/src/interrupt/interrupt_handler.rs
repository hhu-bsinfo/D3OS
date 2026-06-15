pub trait InterruptHandler {
    fn trigger(&self);
}
