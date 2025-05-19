use alloc::{string::String, vec::Vec};
use concurrent::thread::{self};

use crate::{
    build_in::{build_in::BuildIn, clear::ClearBuildIn, echo::EchoBuildIn, exit::ExitBuildIn},
    parser::executable::{Executable, Job},
};

pub struct Executor {}

impl Executor {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn execute(&self, executable: &Executable) -> Result<(), &'static str> {
        for job in &executable.jobs {
            match self.execute_job(&job) {
                Ok(_) => continue,
                Err(msg) => return Err(msg),
            };
        }
        Ok(())
    }

    fn execute_job(&self, job: &Job) -> Result<(), &'static str> {
        let arguments: Vec<&str> = job.arguments.iter().map(String::as_str).collect();
        let thread = match self.try_execute_build_in(&job.command, arguments.clone()) {
            Ok(_) => return Ok(()),
            Err(_) => thread::start_application(&job.command, arguments),
        };
        match thread {
            Some(thread) => thread.join(),
            None => return Err("Command not found!"),
        };
        Ok(())
    }

    fn try_execute_build_in(&self, name: &str, args: Vec<&str>) -> Result<(), ()> {
        match name {
            "echo" => EchoBuildIn::start(args),
            "clear" => ClearBuildIn::start(args),
            "exit" => ExitBuildIn::start(args),
            _ => return Err(()),
        }
    }
}
