use terminal::{print, println};

use crate::built_in::built_in::BuiltIn;

pub struct EchoBuiltIn {}

impl BuiltIn for EchoBuiltIn {
    fn namespace(&self) -> &'static str {
        "echo"
    }

    fn run(&mut self, args: &[&str]) -> isize {
        println!("{}", args.join(" "));
        0
    }
}

impl EchoBuiltIn {
    pub fn new() -> Self {
        Self {}
    }
}
