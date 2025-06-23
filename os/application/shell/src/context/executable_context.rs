use alloc::{
    string::{String, ToString},
    vec::Vec,
};

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

#[derive(Debug, Clone, Default)]
pub struct ExecutableContext {
    pub jobs: Vec<Job>,
}

impl ExecutableContext {
    pub fn new() -> Self {
        ExecutableContext::default()
    }

    pub fn reset(&mut self) {
        *self = ExecutableContext::default()
    }

    pub fn get_jobs(&self) -> &Vec<Job> {
        &self.jobs
    }

    pub fn create_job(&mut self, command: &str) {
        self.jobs.push(Job::new(command.to_string()));
    }

    pub fn add_argument_to_latest_job(&mut self, argument: &str) {
        self.jobs
            .last_mut()
            .expect("Expected at least one job, to add arguments too")
            .add_argument(argument.to_string());
    }
}
