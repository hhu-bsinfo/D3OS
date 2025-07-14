use naming::cwd;
use terminal::println;

use crate::built_in::built_in::BuiltIn;

pub struct PwdBuiltIn {}

impl BuiltIn for PwdBuiltIn {
    fn namespace(&self) -> &'static str {
        "pwd"
    }

    fn run(&mut self, args: &[&str]) -> isize {
        if !args.is_empty() {
            Self::print_usage();
            return -1;
        }
        let Ok(path) = cwd() else {
            Self::print_usage();
            return -1;
        };
        println!("{}", path);
        0
    }
}

impl PwdBuiltIn {
    pub fn new() -> Self {
        Self {}
    }

    fn print_usage() {
        println!("Usage: pwd");
    }
}
