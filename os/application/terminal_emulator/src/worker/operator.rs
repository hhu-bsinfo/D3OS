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
}

impl Worker for Operator {
    fn create(&mut self) {
        if self.thread.is_some() {
            return;
        }
        self.thread = Some(thread::start_application("shell", vec![]).unwrap());
    }

    fn kill(&mut self) {
        if self.thread.is_none() {
            return;
        }
        let _ = syscall(SystemCall::TerminalTerminateOperator, &[1, 0]);
        self.thread = None;
    }
}
