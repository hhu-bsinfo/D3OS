use alloc::{string::String, vec::Vec};
use concurrent::process;

use super::build_in::BuildIn;

pub struct ExitBuildIn {}

pub struct Exit {
    args: Vec<String>,
}

impl BuildIn for ExitBuildIn {
    fn start(args: Vec<&str>) -> Result<(), ()> {
        let exit = Exit::new(args.into_iter().map(String::from).collect());
        exit.run()
    }
}

impl Exit {
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    pub fn run(&self) -> Result<(), ()> {
        process::exit();
        Ok(())
    }
}
