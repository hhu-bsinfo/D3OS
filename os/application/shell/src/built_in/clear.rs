use terminal::print;

use crate::built_in::built_in::BuiltIn;

pub struct ClearBuiltIn {}

impl BuiltIn for ClearBuiltIn {
    fn namespace(&self) -> &'static str {
        "clear"
    }

    fn run(&mut self, _args: &[&str]) -> isize {
        print!("\x1b[2J\x1b[H");
        0
    }
}

impl ClearBuiltIn {
    pub fn new() -> Self {
        Self {}
    }
}
