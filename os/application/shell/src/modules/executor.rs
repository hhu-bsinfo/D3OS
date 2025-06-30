use core::cell::RefCell;

use alloc::{
    format,
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use concurrent::thread;

use crate::{
    build_in::{
        alias::AliasBuildIn, build_in::BuildIn, cd::CdBuildIn, clear::ClearBuildIn, echo::EchoBuildIn,
        exit::ExitBuildIn, mkdir::MkdirBuildIn, pwd::PwdBuildIn, unalias::UnaliasBuildIn,
    },
    context::{
        context::Context,
        executable_context::{Io, Job},
    },
    event::{
        event::Event,
        event_handler::{Error, EventHandler, Response},
    },
    sub_modules::alias::Alias,
};

pub struct Executor {
    alias: Rc<RefCell<Alias>>,
}

impl EventHandler for Executor {
    fn on_submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.execute(clx)
    }
}

impl Executor {
    pub const fn new(alias: Rc<RefCell<Alias>>) -> Self {
        Self { alias }
    }

    pub fn execute(&self, clx: &mut Context) -> Result<Response, Error> {
        for job in clx.executable.get_jobs() {
            if job.input != Io::Std || job.output != Io::Std || job.background_execution {
                return Err(self.handle_unsupported_error(&clx.executable.jobs));
            }
        }

        for job in &clx.executable.jobs {
            match self.execute_job(&job) {
                Ok(_) => continue,
                Err(msg) => return Err(msg),
            };
        }

        clx.events.trigger(Event::PrepareNewLine);
        Ok(Response::Ok)
    }

    fn execute_job(&self, job: &Job) -> Result<Response, Error> {
        let arguments: Vec<&str> = job.arguments.iter().map(String::as_str).collect();

        let thread = match self.try_execute_build_in(&job.command, arguments.clone()) {
            Ok(_) => return Ok(Response::Ok),
            Err(_) => thread::start_application(&job.command, arguments),
        };
        match thread {
            Some(thread) => thread.join(),
            None => return Err(Error::new_inline("Command not found!".to_string(), None)),
        }

        Ok(Response::Ok)
    }

    fn try_execute_build_in(&self, name: &str, args: Vec<&str>) -> Result<(), ()> {
        match name {
            "echo" => EchoBuildIn::start(args),
            "clear" => ClearBuildIn::start(args),
            "exit" => ExitBuildIn::start(args),
            "mkdir" => MkdirBuildIn::start(args),
            "pwd" => PwdBuildIn::start(args),
            "cd" => CdBuildIn::start(args),
            "alias" => AliasBuildIn::new(args, &self.alias).start(),
            "unalias" => UnaliasBuildIn::new(args, &self.alias).start(),
            _ => return Err(()),
        };
        Ok(())
    }

    fn handle_unsupported_error(&self, jobs: &Vec<Job>) -> Error {
        let message = "Pipes, Redirections and background executions are not jet supported by D3OS".to_string();
        let mut hint = "Assume the following execution:\n".to_string();

        for job in jobs {
            let input = match &job.input {
                Io::Std => "StdIn",
                Io::Job(id) => &format!("previous command ({})", jobs[*id].command),
                Io::FileTruncate(file) => &format!("File ({})", file),
                Io::FileAppend(file) => &format!("Append file ({})", file),
            };
            let output = match &job.output {
                Io::Std => "StdOut",
                Io::Job(id) => &format!("next command ({})", jobs[*id].command),
                Io::FileTruncate(file) => &format!("File ({})", file),
                Io::FileAppend(file) => &format!("Append file ({})", file),
            };

            hint.push_str(&format!(
                "Execute: {}, with arguments: {:?}\n\tInput from: {}\n\tOutput to: {}\n\tBackground execution: {:?}\n",
                job.command, job.arguments, input, output, job.background_execution
            ));
        }

        Error::new_inline(message, Some(hint))
    }
}
