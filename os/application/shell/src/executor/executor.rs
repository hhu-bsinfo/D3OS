use alloc::string::String;
use concurrent::thread;

use crate::parser::command_line::{CommandLine, Job};

pub struct Executor {}

impl Executor {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn execute(&self, command_line: CommandLine) {
        for job in command_line.jobs {
            self.execute_job(&job);
        }
    }

    fn execute_job(&self, job: &Job) {
        let args = job.arguments.iter().map(String::as_str).collect();
        thread::start_application(&job.command, args);
    }
}
