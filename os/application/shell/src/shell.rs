#![no_std]

extern crate alloc;

mod build_in;
mod context;
mod event;
mod modules;
mod sub_modules;

use core::cell::RefCell;

use alloc::{boxed::Box, rc::Rc, string::String, vec::Vec};
use logger::info;
use modules::{command_line::CommandLine, executor::Executor, history::History, writer::Writer};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println, read::read_mixed};

use crate::{
    context::context::Context,
    event::{
        event::Event,
        event_handler::{Error, EventHandler},
    },
    modules::{auto_completion::AutoCompletion, parser::parser::Parser},
    sub_modules::{alias::Alias, theme_provider::ThemeProvider},
};

struct Shell {
    clx: Context,
    modules: Vec<Box<dyn EventHandler>>,
    theme_provider: Rc<RefCell<ThemeProvider>>,
}

impl Shell {
    pub fn new() -> Self {
        let alias = Rc::new(RefCell::new(Alias::new()));
        let theme_provider = Rc::new(RefCell::new(ThemeProvider::new()));
        let mut modules: Vec<Box<dyn EventHandler>> = Vec::new();

        modules.push(Box::new(CommandLine::new()));
        modules.push(Box::new(History::new()));
        modules.push(Box::new(Parser::new(alias.clone())));
        modules.push(Box::new(AutoCompletion::new()));
        modules.push(Box::new(Writer::new(theme_provider.clone())));
        modules.push(Box::new(Executor::new(alias.clone(), theme_provider.clone())));

        Self {
            clx: Context::new(),
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
        self.clx.events.trigger(Event::PrepareNewLine);

        loop {
            while let Some(event) = self.clx.events.process() {
                let Err(error) = self.handle_event(&event) else {
                    continue;
                };
                self.handle_error(error);
            }

            self.handle_event(&Event::ProcessCompleted);

            let input_event = self.await_input_event();
            self.handle_event(&input_event);
        }
    }

    fn handle_error(&mut self, error: Error) {
        let theme = self.theme_provider.borrow().get_current();
        let line_break = if error.start_inline { "" } else { "\n" };
        println!(
            "{}{}{}\x1b[0m\n{}{}\x1b[0m",
            line_break,
            theme.error_msg,
            error.message,
            theme.error_hint,
            error.hint.unwrap_or(String::new()),
        );
        self.clx.events.trigger(Event::PrepareNewLine);
    }

    fn handle_event(&mut self, event: &Event) -> Result<(), Error> {
        info!("Events in queue: {:?}", self.clx.events);
        info!("Processing event: {:?}", event);
        for event_handler in &mut self.modules {
            let result = match event {
                Event::KeyPressed(key) => event_handler.on_key_pressed(&mut self.clx, *key),
                Event::CursorMoved(step) => event_handler.on_cursor_moved(&mut self.clx, *step),
                Event::HistoryRestored => event_handler.on_history_restored(&mut self.clx),
                Event::LineWritten => event_handler.on_line_written(&mut self.clx),
                Event::TokensWritten => event_handler.on_tokens_written(&mut self.clx),
                Event::PrepareNewLine => event_handler.on_prepare_next_line(&mut self.clx),
                Event::Submit => event_handler.on_submit(&mut self.clx),
                Event::ProcessCompleted => event_handler.on_process_completed(&mut self.clx),
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
    let mut shell = Shell::new();
    shell.run()
}

// TODO FEAT: Add BuildIn to switch themes
// TODO FEAT: Add working directories!!!
// TODO FEAT: Add help BuildIn
// TODO FEAT: Show && and || executions with build ins (assume extern applications to always succeed)
// TODO FEAT: Add application params to disable optional modules
// TODO FEAT: Pos1 => Cursor to start
// TODO FEAT: End => Cursor to end
// TODO FEAT: ESCAPE => Unfocus suggestion

// TODO IMPROVEMENT: Rework Token creation with less repetition (Assign rules to different kinds??? EolRule, reqCmdRule, ...)
// TODO IMPROVEMENT: Token should accept string in constructor (multi char token are no longer a special case)
// TODO IMPROVEMENT: Limit line len
// TODO IMPROVEMENT: Limit history len
// TODO IMPROVEMENT: Limit alias len
// TODO IMPROVEMENT: ????Move Context into SubModules???? after that rename SubModule to Context
// TODO IMPROVEMENT: Block window_manager execution (show error message how to open window_manager correctly)
// TODO IMPROVEMENT: Move ArgumentKind management into AutoCompletion, remove it from Tokens
// TODO IMPROVEMENT: Restore Lexer, Parser Separation
// TODO IMPROVEMENT: Move mkdir from builtin into application

// TODO FIX: ArgumentKind not updating in terminal
// TODO FIX: Only generic arg suggestion after first generic arg is selected
// TODO FIX: alias and unalias always shows usage error (Problem: builtin accepts only one argument, but alias key="value" has two [key=, "value"])
// TODO FIX: Writer not updating when command history clears line (latest)
// TODO FIX: Show error when line is incomplete (EXCLUDE ArgumentKind)
// TODO FIX: If line is empty but auto completion has focus, pressing backspace doesn't restore terminal cursor position
