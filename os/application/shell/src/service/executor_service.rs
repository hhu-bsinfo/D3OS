use core::cell::RefCell;

use alloc::{rc::Rc, string::String, vec::Vec};
use concurrent::thread;
use terminal::DecodedKey;

use crate::{
    build_in::{
        alias::AliasBuildIn, build_in::BuildIn, cd::CdBuildIn, clear::ClearBuildIn,
        echo::EchoBuildIn, exit::ExitBuildIn, mkdir::MkdirBuildIn, pwd::PwdBuildIn,
        unalias::UnaliasBuildIn,
    },
    context::Context,
    executable::Job,
    sub_service::alias_sub_service::AliasSubService,
};

use super::service::{Service, ServiceError};

pub struct ExecutorService {
    alias: Rc<RefCell<AliasSubService>>,
}

impl Service for ExecutorService {
    fn run(&mut self, event: DecodedKey, context: &mut Context) -> Result<(), ServiceError> {
        match event {
            DecodedKey::Unicode('\n') => self.execute(context),
            _ => Ok(()),
        }
    }
}

impl ExecutorService {
    pub const fn new(alias: Rc<RefCell<AliasSubService>>) -> Self {
        Self { alias }
    }

    pub fn execute(&self, context: &Context) -> Result<(), ServiceError> {
        let executable = match &context.executable {
            Some(executable) => executable,
            None => return Err(ServiceError::new("No executable", None, None)),
        };

        for job in &executable.jobs {
            match self.execute_job(&job) {
                Ok(_) => continue,
                Err(msg) => return Err(msg),
            };
        }
        Ok(())
    }

    fn execute_job(&self, job: &Job) -> Result<(), ServiceError> {
        let arguments: Vec<&str> = job.arguments.iter().map(String::as_str).collect();
        let thread = match self.try_execute_build_in(&job.command, arguments.clone()) {
            Ok(_) => return Ok(()),
            Err(_) => thread::start_application(&job.command, arguments),
        };
        match thread {
            Some(thread) => thread.join(),
            None => return Err(ServiceError::new("Command not found!", None, None)),
        };
        Ok(())
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
}
