#![no_std]

extern crate alloc;

mod build_in;
mod context;
mod controller;
mod executable;
mod service;
mod sub_service;

use core::cell::RefCell;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use context::Context;
#[allow(unused_imports)]
use runtime::*;
use service::{
    command_line_service::CommandLineService,
    drawer_service::DrawerService,
    executor_service::ExecutorService,
    history_service::HistoryService,
    lexer_service::LexerService,
    parser_service::ParserService,
    service::{Error, Service},
};
use sub_service::alias_sub_service::AliasSubService;
use terminal::{DecodedKey, KeyCode, print, println, read::read_mixed};

use crate::service::auto_complete_service::AutoCompleteService;

#[derive(Debug, PartialEq)]
enum ShellState {
    Prepare,
    AwaitUserInput,
}

pub enum Event {
    Prepare,
    Submit,
    HistoryUp,
    HistoryDown,
    CursorLeft,
    CursorRight,
    AutoComplete,
    SimpleKey(char),
}

struct Shell {
    state: ShellState,
    context: Context,
    services: Vec<Box<dyn Service>>,
}

impl Shell {
    pub fn new() -> Self {
        let alias_service = Rc::new(RefCell::new(AliasSubService::new()));
        let mut services: Vec<Box<dyn Service>> = Vec::new();

        services.push(Box::new(CommandLineService::new()));
        services.push(Box::new(HistoryService::new()));
        services.push(Box::new(LexerService::new(alias_service.clone())));
        services.push(Box::new(AutoCompleteService::new()));
        services.push(Box::new(DrawerService::new()));
        services.push(Box::new(ParserService::new()));
        services.push(Box::new(ExecutorService::new(alias_service.clone())));

        Self {
            state: ShellState::Prepare,
            context: Context::new(),
            services,
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
        for service in &mut self.services {
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
