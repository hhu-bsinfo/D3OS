use alloc::vec::Vec;
use concurrent::thread::Thread;

pub trait BuildIn {
    fn start(args: Vec<&str>) -> Option<Thread>;
}
