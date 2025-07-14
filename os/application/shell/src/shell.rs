#![no_std]

extern crate alloc;

mod built_in;
mod context;
mod event;
mod modules;
mod token;

use core::cell::RefCell;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use logger::info;
use modules::{command_line::CommandLine, executor::Executor, history::History, writer::Writer};
use runtime::env::Args;
#[allow(unused_imports)]
use runtime::*;
use terminal::{println, read::read_mixed};

use crate::{
    context::{
        alias_context::AliasContext, executable_context::ExecutableContext, line_context::LineContext,
        suggestion_context::SuggestionContext, theme_context::ThemeContext, tokens_context::TokensContext,
        working_directory_context::WorkingDirectoryContext,
    },
    event::{
        event::Event,
        event_bus::EventBus,
        event_handler::{Error, EventHandler},
    },
    modules::{auto_completion::AutoCompletion, lexer::Lexer, parser::Parser},
};

#[derive(Debug, Default)]
struct Config {
    no_history: bool,
    no_auto_completion: bool,
}

impl Config {
    fn from_args(mut args: Args) -> Result<Self, ()> {
        let mut cfg = Self::default();

        let _skip_application_name = args.next();
        for arg in args {
            match arg.as_str() {
                "--no-history" => cfg.no_history = true,
                "--no-auto-completion" => cfg.no_auto_completion = true,
                _ => return Err(()),
            }
        }
        Ok(cfg)
    }
}

struct Shell {
    modules: Vec<Box<dyn EventHandler>>,
    theme_provider: Rc<RefCell<ThemeContext>>,
    event_bus: EventBus,
}

impl Shell {
    pub fn new(cfg: Config) -> Self {
        let event_bus = EventBus::new();

        let line_provider = Rc::new(RefCell::new(LineContext::new()));
        let suggestion_provider = Rc::new(RefCell::new(SuggestionContext::new()));
        let tokens_provider = Rc::new(RefCell::new(TokensContext::new()));
        let executable_provider = Rc::new(RefCell::new(ExecutableContext::new()));
        let alias_provider = Rc::new(RefCell::new(AliasContext::new()));
        let theme_provider = Rc::new(RefCell::new(ThemeContext::new()));
        let wd_provider = Rc::new(RefCell::new(WorkingDirectoryContext::new()));

        let mut modules: Vec<Box<dyn EventHandler>> = Vec::new();
        modules.push(Box::new(CommandLine::new(line_provider.clone())));
        if !cfg.no_history {
            modules.push(Box::new(History::new(line_provider.clone())));
        }
        modules.push(Box::new(Lexer::new(
            line_provider.clone(),
            tokens_provider.clone(),
            alias_provider.clone(),
        )));
        if !cfg.no_auto_completion {
            modules.push(Box::new(AutoCompletion::new(
                line_provider.clone(),
                tokens_provider.clone(),
                suggestion_provider.clone(),
            )));
        }
        modules.push(Box::new(Writer::new(
            line_provider.clone(),
            tokens_provider.clone(),
            suggestion_provider.clone(),
            theme_provider.clone(),
            wd_provider.clone(),
        )));
        modules.push(Box::new(Parser::new(
            tokens_provider.clone(),
            executable_provider.clone(),
            wd_provider.clone(),
        )));
        modules.push(Box::new(Executor::new(
            executable_provider.clone(),
            &alias_provider,
            &theme_provider,
            &wd_provider,
        )));

        Self {
            event_bus,
            modules,
            theme_provider,
        }
    }

    fn await_input_event(&mut self) -> Event {
        loop {
            let Some(key) = read_mixed() else {
                continue;
            };
            return Event::KeyPressed(key);
        }
    }

    pub fn run(&mut self) {
        self.event_bus.trigger(Event::PrepareNewLine);

        loop {
            while let Some(event) = self.event_bus.process() {
                let Err(error) = self.handle_event(event) else {
                    continue;
                };
                self.event_bus.clear();
                self.event_bus.trigger(Event::ProcessFailed(error));
            }

            self.handle_event(Event::ProcessCompleted);

            let input_event = self.await_input_event();
            self.handle_event(input_event);
        }
    }

    fn handle_event(&mut self, event: Event) -> Result<(), Error> {
        info!("Events in queue: {:?}", self.event_bus);
        info!("Processing event: {:?}", event);
        for event_handler in &mut self.modules {
            let result = match event {
                Event::KeyPressed(key) => event_handler.on_key_pressed(&mut self.event_bus, key),
                Event::CursorMoved(step) => event_handler.on_cursor_moved(&mut self.event_bus, step),
                Event::HistoryRestored => event_handler.on_history_restored(&mut self.event_bus),
                Event::LineWritten => event_handler.on_line_written(&mut self.event_bus),
                Event::TokensWritten => event_handler.on_tokens_written(&mut self.event_bus),
                Event::PrepareNewLine => event_handler.on_prepare_next_line(&mut self.event_bus),
                Event::Submit => event_handler.on_submit(&mut self.event_bus),
                Event::ProcessCompleted => event_handler.on_process_completed(&mut self.event_bus),
                Event::ProcessFailed(ref error) => event_handler.on_process_failed(&mut self.event_bus, error),
            };

            if result.is_err() {
                return Err(result.unwrap_err());
            }
        }
        info!("-------------------------------------------");
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let args = env::args();
    let Ok(cfg) = Config::from_args(args) else {
        println!("Usage: shell [--no-history] [--no-auto-completion]");
        return;
    };

    let mut shell = Shell::new(cfg);
    shell.run()
}
