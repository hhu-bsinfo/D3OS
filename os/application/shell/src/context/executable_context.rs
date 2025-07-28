use alloc::{string::String, vec::Vec};
use logger::warn;

#[derive(Debug, Clone, PartialEq)]
pub enum IoTarget {
    Std,
    Job(usize),
    FileTruncate(String),
    FileAppend(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum JobResult {
    Success,
    Error,
}

impl JobResult {
    pub fn is_success(&self) -> bool {
        *self == JobResult::Success
    }

    pub fn is_error(&self) -> bool {
        *self == JobResult::Error
    }
}

#[derive(Debug, Clone)]
pub struct Executable {
    pub id: usize,
    pub command: String,
    pub arguments: Vec<String>,
    pub input: IoTarget,
    pub output: IoTarget,
    pub background_execution: bool,
    pub requires_executable: Option<(usize, JobResult)>,
}

pub struct ExecutableBuilder {
    id: Option<usize>,
    command: Option<String>,
    arguments: Vec<String>,
    input: IoTarget,
    output: IoTarget,
    background_execution: bool,
    requires_executable: Option<(usize, JobResult)>,
}

impl ExecutableBuilder {
    pub fn new() -> Self {
        Self {
            id: None,
            command: None,
            arguments: Vec::new(),
            input: IoTarget::Std,
            output: IoTarget::Std,
            background_execution: false,
            requires_executable: None,
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

    pub fn use_input(&mut self, input: IoTarget) -> &mut Self {
        self.input = input;
        self
    }

    pub fn use_output(&mut self, output: IoTarget) -> &mut Self {
        warn!("builder: id:{:?} {:?}", self.id, output);
        self.output = output;
        self
    }

    pub fn run_in_background(&mut self, bg_execution: bool) -> &mut Self {
        self.background_execution = bg_execution;
        self
    }

    pub fn requires_executable(&mut self, id: usize, result: JobResult) -> &mut Self {
        self.requires_executable = Some((id, result));
        self
    }

    pub fn peek_id(&self) -> Option<usize> {
        self.id
    }

    pub fn peek_command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    pub fn build(&self) -> Result<Executable, &'static str> {
        if self.id.is_none() {
            return Err("Id is required");
        }
        if self.command.is_none() {
            return Err("Command is required");
        }
        Ok(Executable {
            id: self.id.unwrap(),
            command: self.command.as_ref().unwrap().clone(),
            arguments: self.arguments.clone(),
            input: self.input.clone(),
            output: self.output.clone(),
            background_execution: self.background_execution,
            requires_executable: self.requires_executable.clone(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExecutableContext {
    pub executables: Vec<Executable>,
}

impl ExecutableContext {
    pub fn new() -> Self {
        ExecutableContext::default()
    }

    pub fn reset(&mut self) {
        *self = ExecutableContext::default()
    }

    pub fn get_executables(&self) -> &Vec<Executable> {
        &self.executables
    }

    pub fn is_empty(&self) -> bool {
        self.executables.is_empty()
    }

    pub fn len(&self) -> usize {
        self.executables.len()
    }

    pub fn add_executable(&mut self, executable: Executable) {
        self.executables.push(executable);
    }

    pub fn last_executable(&self) -> Option<&Executable> {
        self.executables.last()
    }

    pub fn last_mut_executable(&mut self) -> Option<&mut Executable> {
        self.executables.last_mut()
    }
}
