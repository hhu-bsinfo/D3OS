use alloc::{string::String, vec::Vec};
use naming::mkdir;
use terminal::{print, println};

use super::build_in::BuildIn;

pub struct MkdirBuildIn {}

pub struct Mkdir {
    args: Vec<String>,
}

impl BuildIn for MkdirBuildIn {
    fn start(args: Vec<&str>) -> Result<(), ()> {
        let mkdir = Mkdir::new(args.into_iter().map(String::from).collect());
        mkdir.run()
    }
}

impl Mkdir {
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    pub fn run(&self) -> Result<(), ()> {
        let name = match self.args.get(0) {
            Some(name) => name,
            None => return self.error(),
        };
        match mkdir(name) {
            Ok(_) => Ok(()),
            Err(_) => self.error(),
        }
    }

    fn error(&self) -> Result<(), ()> {
        println!("Usage: mkdir DIRECTORY");
        Err(())
    }
}
