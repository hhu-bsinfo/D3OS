use alloc::{string::String, vec::Vec};

#[derive(Debug, Clone, PartialEq)]
pub enum Io {
    Std,
    Job(usize),
    FileTruncate(String),
    FileAppend(String),
}

#[derive(Debug, Clone)]
pub enum JobResult {
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: usize,
    pub command: String,
    pub arguments: Vec<String>,
    pub input: Io,
    pub output: Io,
    pub background_execution: bool,
    pub requires_job: Option<(usize, JobResult)>,
}

pub struct JobBuilder {
    id: Option<usize>,
    command: Option<String>,
    arguments: Vec<String>,
    input: Io,
    output: Io,
    background_execution: bool,
    requires_job: Option<(usize, JobResult)>,
}

impl JobBuilder {
    pub fn new() -> Self {
        Self {
            id: None,
            command: None,
            arguments: Vec::new(),
            input: Io::Std,
            output: Io::Std,
            background_execution: false,
            requires_job: None,
        }
    }

    pub fn id(&mut self, id: usize) -> &mut Self {
        self.id = Some(id);
        self
    }

    pub fn command(&mut self, command: String) -> &mut Self {
        self.command = Some(command);
        self
    }

    pub fn add_argument(&mut self, arg: String) -> &mut Self {
        self.arguments.push(arg);
        self
    }

    pub fn use_input(&mut self, input: Io) -> &mut Self {
        self.input = input;
        self
    }

    pub fn use_output(&mut self, output: Io) -> &mut Self {
        self.output = output;
        self
    }

    pub fn run_in_background(&mut self, bg_execution: bool) -> &mut Self {
        self.background_execution = bg_execution;
        self
    }

    pub fn requires_job(&mut self, id: usize, result: JobResult) -> &mut Self {
        self.requires_job = Some((id, result));
        self
    }

    pub fn peek_id(&self) -> Option<usize> {
        self.id
    }

    pub fn build(&self) -> Result<Job, &'static str> {
        if self.id.is_none() {
            return Err("Id is required");
        }
        if self.command.is_none() {
            return Err("Command is required");
        }
        Ok(Job {
            id: self.id.unwrap(),
            command: self.command.as_ref().unwrap().clone(),
            arguments: self.arguments.clone(),
            input: self.input.clone(),
            output: self.output.clone(),
            background_execution: self.background_execution,
            requires_job: self.requires_job.clone(),
        })
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

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn add_job(&mut self, job: Job) {
        self.jobs.push(job);
    }

    pub fn last_job(&self) -> Option<&Job> {
        self.jobs.last()
    }

    pub fn last_job_mut(&mut self) -> Option<&mut Job> {
        self.jobs.last_mut()
    }
}
