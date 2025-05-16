use alloc::vec;
use concurrent::thread::{self, Thread};
use syscall::{SystemCall, syscall};

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

    pub fn kill(&mut self) {
        if self.thread.is_none() {
            return;
        }
        let _ = syscall(SystemCall::TerminalTerminateOperator, &[1, 0]);
        self.thread = None;
    }
}
