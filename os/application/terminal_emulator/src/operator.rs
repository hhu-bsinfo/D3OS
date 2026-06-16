use alloc::vec;
use log::info;
use concurrent::thread::{self, Thread};

pub struct Operator {
    thread: Option<Thread>,
}

impl Operator {
    pub const fn new() -> Self {
        Self { thread: None }
    }
    
    /// Start the shell.
    /// 
    /// This happens in a separate thread, so it can wait for the shell to exit
    /// (or crash) and then restart it.
    pub fn create(&mut self) {
        assert!(self.thread.is_none());
        self.thread = thread::create(|| loop {
            thread::start_application("shell", vec![])
                .expect("Unable to start operator")
                .join();
            info!("Restarting shell...");
        });
    }
}
