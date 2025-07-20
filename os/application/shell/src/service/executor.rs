use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};
use concurrent::thread;
use logger::warn;
use terminal::println;

use crate::{
    built_in::{
        alias::AliasBuiltIn, built_in::BuiltIn, cd::CdBuiltIn, clear::ClearBuiltIn, debug_error::DebugErrorBuiltIn,
        debug_success::DebugSuccessBuiltIn, echo::EchoBuiltIn, exit::ExitBuiltIn, help::HelpBuiltIn,
        mkdir::MkdirBuiltIn, pwd::PwdBuiltIn, theme::ThemeBuiltIn, unalias::UnaliasBuiltIn,
        window_manager::WindowManagerBuiltIn,
    },
    context::{
        alias_context::AliasContext,
        context::ContextProvider,
        executable_context::{Executable, ExecutableContext, IoTarget},
        theme_context::ThemeContext,
        working_directory_context::WorkingDirectoryContext,
    },
    event::{
        event::Event,
        event_bus::EventBus,
        event_handler::{Error, EventHandler, Response},
    },
};

pub struct ExecutorService {
    executable_provider: ContextProvider<ExecutableContext>,

    built_ins: Vec<Box<dyn BuiltIn>>,
}

impl EventHandler for ExecutorService {
    fn on_submit(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        self.execute(event_bus)
    }
}

impl ExecutorService {
    pub fn new(
        executable_provider: ContextProvider<ExecutableContext>,
        alias_provider: &ContextProvider<AliasContext>,
        theme_provider: &ContextProvider<ThemeContext>,
        wd_provider: &ContextProvider<WorkingDirectoryContext>,
    ) -> Self {
        let mut built_ins: Vec<Box<dyn BuiltIn>> = Vec::new();
        built_ins.push(Box::new(AliasBuiltIn::new(alias_provider.clone())));
        built_ins.push(Box::new(CdBuiltIn::new(wd_provider.clone())));
        built_ins.push(Box::new(ClearBuiltIn::new()));
        built_ins.push(Box::new(EchoBuiltIn::new()));
        built_ins.push(Box::new(ExitBuiltIn::new()));
        built_ins.push(Box::new(MkdirBuiltIn::new(wd_provider.clone())));
        built_ins.push(Box::new(PwdBuiltIn::new(wd_provider.clone())));
        built_ins.push(Box::new(ThemeBuiltIn::new(theme_provider.clone())));
        built_ins.push(Box::new(UnaliasBuiltIn::new(alias_provider.clone())));
        built_ins.push(Box::new(WindowManagerBuiltIn::new()));
        built_ins.push(Box::new(DebugSuccessBuiltIn::new()));
        built_ins.push(Box::new(DebugErrorBuiltIn::new()));
        built_ins.push(Box::new(HelpBuiltIn::new()));

        Self {
            executable_provider,
            built_ins,
        }
    }

    fn execute(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        let executables = { self.executable_provider.borrow().get_executables().clone() };
        warn!("{:#?}", executables);
        // Check if executables contain unsupported operations
        for executable in &executables {
            if executable.input != IoTarget::Std
                || executable.output != IoTarget::Std
                || executable.background_execution
            {
                return Err(self.handle_unsupported_error(&executables));
            }
        }

        let mut exit_codes = Vec::with_capacity(executables.len());
        for executable in executables {
            if Self::should_stop_on_dependency(&executable, &exit_codes) {
                break;
            }

            let exit_code = self.execute_executable(&executable);
            exit_codes.push(exit_code);
        }

        event_bus.trigger(Event::PrepareNewLine);
        Ok(Response::Ok)
    }

    fn execute_executable(&mut self, executable: &Executable) -> isize {
        let args: Vec<&str> = executable.arguments.iter().map(String::as_str).collect();

        if let Ok(built_in_exit_code) = self.execute_build_in(&executable.command, &args) {
            return built_in_exit_code;
        }

        let Some(thread) = thread::start_application(&executable.command, args) else {
            println!("Command not found: {}", &executable.command);
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

    fn handle_unsupported_error(&self, executables: &Vec<Executable>) -> Error {
        let message = "Pipes, Redirections and background executions are not jet supported by D3OS".to_string();
        let mut hint = "Assume the following execution:\n".to_string();

        for executable in executables {
            let input = match &executable.input {
                IoTarget::Std => "StdIn",
                IoTarget::Job(id) => &format!("previous command ({})", executables[*id].command),
                IoTarget::FileTruncate(file) => &format!("File ({})", file),
                IoTarget::FileAppend(file) => &format!("Append file ({})", file),
            };
            let output = match &executable.output {
                IoTarget::Std => "StdOut",
                IoTarget::Job(id) => &format!("next command ({})", executables[*id].command),
                IoTarget::FileTruncate(file) => &format!("File ({})", file),
                IoTarget::FileAppend(file) => &format!("Append file ({})", file),
            };

            hint.push_str(&format!(
                "Execute: {}, with arguments: {:?}\n\tInput from: {}\n\tOutput to: {}\n\tBackground execution: {:?}\n",
                executable.command, executable.arguments, input, output, executable.background_execution
            ));
        }

        Error::new_mid_execution(message, Some(hint))
    }

    fn should_stop_on_dependency(executable: &Executable, exit_codes: &[isize]) -> bool {
        if let Some((idx, res)) = &executable.requires_executable {
            if let Some(&prev_code) = exit_codes.get(*idx) {
                return (res.is_success() && prev_code < 0) || (res.is_error() && prev_code >= 0);
            }
        }
        false
    }
}
