use naming::mkdir;
use terminal::println;

use crate::built_in::built_in::BuiltIn;

pub struct MkdirBuiltIn {}

impl BuiltIn for MkdirBuiltIn {
    fn namespace(&self) -> &'static str {
        "mkdir"
    }

    fn run(&mut self, args: &[&str]) -> isize {
        let Some(path) = args.get(0) else {
            Self::print_usage();
            return -1;
        };
        if mkdir(path).is_err() {
            Self::print_usage();
            return -1;
        }
        0
    }
}

impl MkdirBuiltIn {
    pub fn new() -> Self {
        Self {}
    }

    fn print_usage() {
        println!("Usage: mkdir DIRECTORY");
    }
}
