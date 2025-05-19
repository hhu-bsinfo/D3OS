use alloc::{string::String, vec::Vec};
use concurrent::thread::{self, Thread};
use terminal::print;

use super::{
    args_cache::{cache_args, flush_args},
    build_in::BuildIn,
};

pub struct ClearBuildIn {}

pub struct Clear {
    args: Vec<String>,
}

impl BuildIn for ClearBuildIn {
    fn start(args: Vec<&str>) -> Option<Thread> {
        let thread = thread::create(|| {
            let args = flush_args(thread::current().unwrap().id());
            let echo = Clear::new(args.to_vec());
            echo.run();
        });

        thread.inspect(|t| cache_args(t.id(), args))
    }
}

impl Clear {
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    pub fn run(&self) {
        print!("\x1b[2J\x1b[H");
    }
}
