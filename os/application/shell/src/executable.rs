use alloc::{string::String, vec::Vec};

#[derive(Debug, PartialEq, Clone)]
pub struct Job {
    pub command: String,
    pub arguments: Vec<String>,
}

impl Job {
    pub const fn new(command: String) -> Self {
        Self {
            command,
            arguments: Vec::new(),
        }
    }

    pub fn add_argument(&mut self, argument: String) {
        self.arguments.push(argument);
    }
}

#[derive(Debug, Clone)]
pub struct Executable {
    pub jobs: Vec<Job>,
}

impl Executable {
    pub const fn new() -> Self {
        Self { jobs: Vec::new() }
    }

    pub fn create_job(&mut self, command: String) {
        self.jobs.push(Job::new(command));
    }

    pub fn add_argument_to_latest_job(&mut self, argument: String) {
        self.jobs
            .last_mut()
            .expect("Expected at least one job, to add arguments too")
            .add_argument(argument);
    }
}
