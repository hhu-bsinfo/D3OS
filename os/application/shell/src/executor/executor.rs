use core::cell::RefCell;

use alloc::{rc::Rc, string::String};
use concurrent::thread;
use terminal::{print, println};

use crate::{
    module::Module,
    parser::command_line::{CommandLine, Job},
    state::State,
};

pub struct Executor {
    state: Rc<RefCell<State>>,
}

impl Executor {
    pub const fn new(state: Rc<RefCell<State>>) -> Self {
        Self { state }
    }

    pub fn execute(&self, command_line: &CommandLine) {
        for job in &command_line.jobs {
            self.execute_job(&job);
        }
    }

    fn execute_job(&self, job: &Job) {
        let args = job.arguments.iter().map(String::as_str).collect();
        match thread::start_application(&job.command, args) {
            Some(thread) => thread.join(),
            None => println!("Command not found!"),
        };
    }
}

impl Module for Executor {
    fn run(&mut self) {
        let state = self.state.borrow();
        let command_line = match &state.command_line {
            Some(command_line) => command_line,
            None => return,
        };

        self.execute(command_line);
    }
}
