use concurrent::process;

use crate::built_in::built_in::BuiltIn;

pub struct ExitBuiltIn {}

impl BuiltIn for ExitBuiltIn {
    fn namespace(&self) -> &'static str {
        "exit"
    }

    fn run(&mut self, _args: &[&str]) -> isize {
        process::exit();
        0
    }
}

impl ExitBuiltIn {
    pub fn new() -> Self {
        Self {}
    }
}
