use alloc::{string::String, vec::Vec};
use naming::cd;
use terminal::{print, println};

use super::build_in::BuildIn;

pub struct CdBuildIn {}

pub struct Cd {
    args: Vec<String>,
}

impl BuildIn for CdBuildIn {
    fn start(args: Vec<&str>) -> Result<(), ()> {
        let cd = Cd::new(args.into_iter().map(String::from).collect());
        cd.run()
    }
}

impl Cd {
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    pub fn run(&self) -> Result<(), ()> {
        let directory = match self.args.get(0) {
            Some(name) => name,
            None => return self.error(),
        };
        match cd(directory) {
            Ok(_) => Ok(()),
            Err(_) => self.error(),
        }
    }

    fn error(&self) -> Result<(), ()> {
        println!("Usage: cd DIRECTORY");
        Err(())
    }
}
