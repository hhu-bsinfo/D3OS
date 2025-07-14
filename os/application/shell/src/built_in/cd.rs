use naming::cd;
use terminal::println;

use crate::built_in::built_in::BuiltIn;

pub struct CdBuiltIn {}

impl BuiltIn for CdBuiltIn {
    fn namespace(&self) -> &'static str {
        "cd"
    }

    fn run(&mut self, args: &[&str]) -> isize {
        let Some(dir) = args.get(0) else {
            Self::print_usage();
            return -1;
        };
        if cd(dir).is_err() {
            return -1;
        }

        0
    }
}

impl CdBuiltIn {
    pub fn new() -> Self {
        Self {}
    }

    fn print_usage() {
        println!("Usage: cd DIRECTORY");
    }
}
