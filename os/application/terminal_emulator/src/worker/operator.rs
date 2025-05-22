use alloc::vec;
use concurrent::thread::{self, Thread};
use syscall::{SystemCall, syscall};

use crate::worker::worker::Worker;

pub struct Operator {
    thread: Option<Thread>,
}

impl Operator {
    pub const fn new() -> Self {
        Self { thread: None }
    }

    pub fn create(&mut self) {
        if self.thread.is_some() {
            return;
        }
        self.thread = Some(thread::start_application("shell", vec![]).unwrap());
    }
}
