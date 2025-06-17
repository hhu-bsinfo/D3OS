#![no_std]

extern crate alloc;

mod build_in;
mod context;
mod event;
mod executable;
mod modules;

use core::cell::RefCell;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use context::Context;
use modules::{
    command_line::CommandLine, executor::Executor, history::History, lexer::Lexer, parser::Parser, writer::Writer,
};
#[allow(unused_imports)]
use runtime::*;
use terminal::{DecodedKey, KeyCode, print, println, read::read_mixed};

use crate::{
    event::{
        event::Event,
        event_handler::{Error, EventHandler},
    },
    modules::{alias::Alias, auto_completion::AutoCompletion},
};

#[derive(Debug, PartialEq)]
enum ShellState {
    Prepare,
    AwaitUserInput,
}

struct Shell {
    state: ShellState,
    context: Context,
    modules: Vec<Box<dyn EventHandler>>,
}

impl Shell {
    pub fn new() -> Self {
        let alias = Rc::new(RefCell::new(Alias::new()));
        let mut modules: Vec<Box<dyn EventHandler>> = Vec::new();

        modules.push(Box::new(CommandLine::new()));
        modules.push(Box::new(History::new()));
        modules.push(Box::new(Lexer::new(alias.clone())));
        modules.push(Box::new(AutoCompletion::new()));
        modules.push(Box::new(Lexer::new(alias.clone()))); // TODO WORKAROUND (autocompletion writes to line, which means tokens need to be revalidated to show changes)
        modules.push(Box::new(Writer::new()));
        modules.push(Box::new(Parser::new()));
        modules.push(Box::new(Executor::new(alias.clone())));

        Self {
            state: ShellState::Prepare,
            context: Context::new(),
            modules,
        }
    }

    fn get_event(&mut self) -> Option<Event> {
        if self.state == ShellState::Prepare {
            return Some(Event::Prepare);
        }

        match read_mixed() {
            Some(DecodedKey::Unicode('\n')) => Some(Event::Submit),
            Some(DecodedKey::Unicode('\t')) => Some(Event::AutoComplete),
            Some(DecodedKey::Unicode(ch)) => Some(Event::SimpleKey(ch)),
            Some(DecodedKey::RawKey(KeyCode::ArrowUp)) => Some(Event::HistoryUp),
            Some(DecodedKey::RawKey(KeyCode::ArrowDown)) => Some(Event::HistoryDown),
            Some(DecodedKey::RawKey(KeyCode::ArrowLeft)) => Some(Event::CursorLeft),
            Some(DecodedKey::RawKey(KeyCode::ArrowRight)) => Some(Event::CursorRight),
            _ => None,
        }
    }

    pub fn run(&mut self) {
        loop {
            let event = self.get_event();
            let result = match &event {
                Some(event) => self.handle_event(&event),
                None => continue,
            };
            match result {
                Ok(_) => self.handle_success(&event.unwrap()),
                Err(error) => self.handle_error(error),
            }
        }
    }

    fn handle_success(&mut self, event: &Event) {
        match event {
            Event::Prepare => self.state = ShellState::AwaitUserInput,
            Event::Submit => self.state = ShellState::Prepare,
            _ => (),
        }
    }

    fn handle_error(&mut self, error: Error) {
        println!("{}", error.message);
        self.state = ShellState::Prepare;
    }

    fn handle_event(&mut self, event: &Event) -> Result<(), Error> {
        for service in &mut self.modules {
            let result = match event {
                Event::Prepare => service.prepare(&mut self.context),
                Event::Submit => service.submit(&mut self.context),
                Event::HistoryUp => service.history_up(&mut self.context),
                Event::HistoryDown => service.history_down(&mut self.context),
                Event::CursorLeft => service.cursor_left(&mut self.context),
                Event::CursorRight => service.cursor_right(&mut self.context),
                Event::AutoComplete => service.auto_complete(&mut self.context),
                Event::SimpleKey(key) => service.simple_key(&mut self.context, *key),
            };

            if result.is_err() {
                return Err(result.unwrap_err());
            }
        }
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut shell = Shell::new();
    shell.run()
}
