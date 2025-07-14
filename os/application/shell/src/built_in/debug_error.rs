use terminal::println;

use crate::built_in::built_in::BuiltIn;

pub struct DebugErrorBuiltIn {}

impl BuiltIn for DebugErrorBuiltIn {
    fn namespace(&self) -> &'static str {
        "debug_error"
    }

    fn run(&mut self, _args: &[&str]) -> isize {
        println!("Debug: Returning exit code -1");
        -1
    }
}

impl DebugErrorBuiltIn {
    pub fn new() -> Self {
        Self {}
    }
}
