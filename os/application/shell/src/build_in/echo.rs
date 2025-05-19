use alloc::{string::String, vec::Vec};
use concurrent::thread::{self, Thread};
use terminal::{print, println};

use super::{
    args_cache::{cache_args, flush_args},
    build_in::BuildIn,
};

pub struct EchoBuildIn {}

pub struct Echo {
    args: Vec<String>,
}

impl BuildIn for EchoBuildIn {
    fn start(args: Vec<&str>) -> Option<Thread> {
        let thread = thread::create(|| {
            let args = flush_args(thread::current().unwrap().id());
            let echo = Echo::new(args.to_vec());
            echo.run();
        });

        thread.inspect(|t| cache_args(t.id(), args))
    }
}

impl Echo {
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    pub fn run(&self) {
        println!("{}", self.args.join(" "));
    }
}
