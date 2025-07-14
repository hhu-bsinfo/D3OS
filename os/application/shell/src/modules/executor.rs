use core::cell::RefCell;

use alloc::{
    boxed::Box,
    format,
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use concurrent::thread;
use terminal::{print, println};

use crate::{
    built_in::{
        alias::AliasBuiltIn, built_in::BuiltIn, cd::CdBuiltIn, clear::ClearBuiltIn, debug_error::DebugErrorBuiltIn,
        debug_success::DebugSuccessBuiltIn, echo::EchoBuiltIn, exit::ExitBuiltIn, mkdir::MkdirBuiltIn, pwd::PwdBuiltIn,
        theme::ThemeBuiltIn, unalias::UnaliasBuiltIn, window_manager::WindowManagerBuiltIn,
    },
    context::{
        alias_context::AliasContext,
        executable_context::{ExecutableContext, Io, Job},
        theme_context::ThemeContext,
    },
    event::{
        event::Event,
        event_bus::EventBus,
        event_handler::{Error, EventHandler, Response},
    },
};

pub struct Executor {
    executable_provider: Rc<RefCell<ExecutableContext>>,

    built_ins: Vec<Box<dyn BuiltIn>>,
}

impl EventHandler for Executor {
    fn on_submit(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        self.execute(event_bus)
    }
}

impl Executor {
    pub fn new(
        executable_provider: Rc<RefCell<ExecutableContext>>,
        alias_provider: Rc<RefCell<AliasContext>>,
        theme_provider: Rc<RefCell<ThemeContext>>,
    ) -> Self {
        let mut built_ins: Vec<Box<dyn BuiltIn>> = Vec::new();
        built_ins.push(Box::new(AliasBuiltIn::new(alias_provider.clone())));
        built_ins.push(Box::new(CdBuiltIn::new()));
        built_ins.push(Box::new(ClearBuiltIn::new()));
        built_ins.push(Box::new(EchoBuiltIn::new()));
        built_ins.push(Box::new(ExitBuiltIn::new()));
        built_ins.push(Box::new(MkdirBuiltIn::new()));
        built_ins.push(Box::new(PwdBuiltIn::new()));
        built_ins.push(Box::new(ThemeBuiltIn::new(theme_provider.clone())));
        built_ins.push(Box::new(UnaliasBuiltIn::new(alias_provider.clone())));
        built_ins.push(Box::new(WindowManagerBuiltIn::new()));
        built_ins.push(Box::new(DebugSuccessBuiltIn::new()));
        built_ins.push(Box::new(DebugErrorBuiltIn::new()));

        Self {
            executable_provider,
            built_ins,
        }
    }

    fn execute(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        let jobs = { self.executable_provider.borrow().get_jobs().clone() };

        // Check if jobs contain unsupported operations
        for job in &jobs {
            if job.input != Io::Std || job.output != Io::Std || job.background_execution {
                return Err(self.handle_unsupported_error(&jobs));
            }
        }

        let mut exit_codes = Vec::with_capacity(jobs.len());
        for job in jobs {
            if Self::should_stop_on_dependency(&job, &exit_codes) {
                break;
            }

            let exit_code = self.execute_job(&job);
            exit_codes.push(exit_code);
        }

        event_bus.trigger(Event::PrepareNewLine);
        Ok(Response::Ok)
    }

    fn execute_job(&mut self, job: &Job) -> isize {
        let args: Vec<&str> = job.arguments.iter().map(String::as_str).collect();

        if let Ok(built_in_exit_code) = self.execute_build_in(&job.command, &args) {
            return built_in_exit_code;
        }

        let Some(thread) = thread::start_application(&job.command, args) else {
            println!("Command not found: {}", &job.command);
            return -1;
        };
        // Extern applications don't yet provide a exit code => We assume success
        thread.join();
        0
    }

    fn execute_build_in(&mut self, cmd: &str, args: &[&str]) -> Result<isize, ()> {
        self.built_ins
            .iter_mut()
            .find(|built_in| built_in.namespace() == cmd)
            .map(|built_in| built_in.run(args))
            .ok_or(())
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

        Error::new_mid_execution(message, Some(hint))
    }

    fn should_stop_on_dependency(job: &Job, exit_codes: &[isize]) -> bool {
        if let Some((idx, res)) = &job.requires_job {
            if let Some(&prev_code) = exit_codes.get(*idx) {
                return (res.is_success() && prev_code < 0) || (res.is_error() && prev_code >= 0);
            }
        }
        false
    }
}
