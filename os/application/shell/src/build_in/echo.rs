use alloc::{string::String, vec::Vec};
use terminal::{print, println};

use super::build_in::BuildIn;

pub struct EchoBuildIn {}

pub struct Echo {
    args: Vec<String>,
}

impl BuildIn for EchoBuildIn {
    fn start(args: Vec<&str>) -> Result<(), ()> {
        let echo = Echo::new(args.into_iter().map(String::from).collect());
        echo.run();
        Ok(())
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
