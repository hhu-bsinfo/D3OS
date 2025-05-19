use alloc::{string::String, vec::Vec};
use terminal::print;

use super::build_in::BuildIn;

pub struct ClearBuildIn {}

pub struct Clear {
    args: Vec<String>,
}

impl BuildIn for ClearBuildIn {
    fn start(args: Vec<&str>) -> Result<(), ()> {
        let echo = Clear::new(args.into_iter().map(String::from).collect());
        echo.run();
        Ok(())
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
