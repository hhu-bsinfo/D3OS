#![no_std]

extern crate alloc;

mod build_in;
mod context;
mod event;
mod modules;

use core::cell::RefCell;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use logger::warn;
use modules::{
    command_line::CommandLine, executor::Executor, history::History, lexer::Lexer, parser::Parser, writer::Writer,
};
#[allow(unused_imports)]
use runtime::*;
use terminal::{DecodedKey, KeyCode, print, println, read::read_mixed};

use crate::{
    context::context::Context,
    event::{
        event::Event,
        event_handler::{Error, EventHandler},
    },
    modules::{alias::Alias, auto_completion::AutoCompletion},
};

struct Shell {
    clx: Context,
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
            clx: Context::new(),
            modules,
        }
    }

    fn await_input_event(&mut self) -> Event {
        loop {
            return match read_mixed() {
                Some(DecodedKey::Unicode('\n')) => Event::Submit,
                Some(DecodedKey::Unicode('\t')) => Event::AutoComplete,
                Some(DecodedKey::Unicode(ch)) => Event::SimpleKey(ch),
                Some(DecodedKey::RawKey(KeyCode::ArrowUp)) => Event::HistoryUp,
                Some(DecodedKey::RawKey(KeyCode::ArrowDown)) => Event::HistoryDown,
                Some(DecodedKey::RawKey(KeyCode::ArrowLeft)) => Event::CursorLeft,
                Some(DecodedKey::RawKey(KeyCode::ArrowRight)) => Event::CursorRight,
                _ => continue,
            };
        }
    }

    pub fn run(&mut self) {
        self.clx.events.trigger(Event::PrepareNewLine);

        loop {
            let event = match self.clx.events.process() {
                Some(event) => event,
                None => self.await_input_event(),
            };
            warn!("Processing event: {:?}", event);

            let Err(error) = self.handle_event(&event) else {
                continue;
            };
            self.handle_error(error);
        }
    }

    fn handle_error(&mut self, error: Error) {
        println!("{}", error.message);
        self.clx.events.trigger(Event::PrepareNewLine);
    }

    fn handle_event(&mut self, event: &Event) -> Result<(), Error> {
        for service in &mut self.modules {
            let result = match event {
                Event::PrepareNewLine => service.prepare(&mut self.clx),
                Event::Submit => service.submit(&mut self.clx),
                Event::HistoryUp => service.history_up(&mut self.clx),
                Event::HistoryDown => service.history_down(&mut self.clx),
                Event::CursorLeft => service.cursor_left(&mut self.clx),
                Event::CursorRight => service.cursor_right(&mut self.clx),
                Event::AutoComplete => service.auto_complete(&mut self.clx),
                Event::SimpleKey(key) => service.simple_key(&mut self.clx, *key),
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
