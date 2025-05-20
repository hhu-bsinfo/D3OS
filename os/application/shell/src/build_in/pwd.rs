use alloc::{string::String, vec::Vec};
use naming::cwd;
use terminal::{print, println};

use super::build_in::BuildIn;

pub struct PwdBuildIn {}

pub struct Pwd {
    args: Vec<String>,
}

impl BuildIn for PwdBuildIn {
    fn start(args: Vec<&str>) -> Result<(), ()> {
        let pwd = Pwd::new(args.into_iter().map(String::from).collect());
        pwd.run()
    }
}

impl Pwd {
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    pub fn run(&self) -> Result<(), ()> {
        if !self.args.is_empty() {
            return self.error();
        }

        let path = match cwd() {
            Ok(path) => path,
            Err(_) => return self.error(),
        };

        println!("{}", path);
        Ok(())
    }

    fn error(&self) -> Result<(), ()> {
        println!("Usage: pwd");
        Err(())
    }
}
