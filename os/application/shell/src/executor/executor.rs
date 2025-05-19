use alloc::string::String;
use concurrent::thread;

use crate::parser::executable::{Executable, Job};

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
        let args = job.arguments.iter().map(String::as_str).collect();
        match thread::start_application(&job.command, args) {
            Some(thread) => thread.join(),
            None => return Err("Command not found!"),
        };
        Ok(())
    }
}
