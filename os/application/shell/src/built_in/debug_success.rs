use terminal::println;

use crate::built_in::built_in::BuiltIn;

pub struct DebugSuccessBuiltIn {}

impl BuiltIn for DebugSuccessBuiltIn {
    fn namespace(&self) -> &'static str {
        "debug_success"
    }

    fn run(&mut self, _args: &[&str]) -> isize {
        println!("Debug: Returning exit code 0");
        0
    }
}

impl DebugSuccessBuiltIn {
    pub fn new() -> Self {
        Self {}
    }
}
