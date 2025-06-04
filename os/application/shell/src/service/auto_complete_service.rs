use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use globals::application::{APPLICATION_REGISTRY, Application};
use logger::warn;

use crate::{
    context::Context,
    service::{
        lexer_service::Token,
        service::{Error, Response, Service},
    },
};

#[derive(Debug)]
pub struct AutoCompleteService {
    applications: Vec<Application>,
    current_index: usize,
}

impl Service for AutoCompleteService {
    fn auto_complete(&mut self, context: &mut Context) -> Result<Response, Error> {
        if !context.is_autocomplete_active {
            self.activate(context);
        }

        if !context.line_suffix.is_empty() {
            return Ok(Response::Ok);
        }

        self.cycle(context)
    }

    fn simple_key(&mut self, context: &mut Context, key: char) -> Result<Response, Error> {
        if key == ' ' {
            return self.adopt(context);
        }
        self.reset(context);
        self.cycle(context)
    }

    fn prepare(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset(context)
    }

    fn submit(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset(context)
    }

    fn cursor_left(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset(context)
    }

    fn cursor_right(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset(context)
    }

    fn history_down(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset(context)
    }

    fn history_up(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset(context)
    }
}

impl AutoCompleteService {
    pub fn new() -> Self {
        Self {
            applications: Vec::from(APPLICATION_REGISTRY.applications),
            current_index: 0,
        }
    }

    fn adopt(&mut self, context: &mut Context) -> Result<Response, Error> {
        if !context.is_cursor_at_end() || context.line_suffix.is_empty() {
            return Ok(Response::Skip);
        }

        let intercept_char = context.line.pop();
        context.line.push_str(&context.line_suffix);
        context
            .line
            .push(intercept_char.expect("Expected command line service to write a char"));

        context.cursor_position += context.line_suffix.len();
        self.reset(context)
    }

    fn reset(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.current_index = 0;

        if context.line_suffix.is_empty() {
            return Ok(Response::Skip);
        }

        context.is_autocomplete_active = false;
        context.set_dirty_offset_from_line_suffix(0);
        context.line_suffix.clear();
        Ok(Response::Ok)
    }

    fn activate(&mut self, context: &mut Context) -> Result<Response, Error> {
        context.is_autocomplete_active = true;
        context.set_dirty_offset_from_line_suffix(0);
        Ok(Response::Ok)
    }

    fn cycle(&mut self, context: &mut Context) -> Result<Response, Error> {
        if !context.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        let completion = match context.tokens.last().cloned() {
            None => self.cycle_command(&String::new()),
            Some(Token::Command(_, cmd)) => self.cycle_command(&cmd),
            Some(Token::Argument(_, arg)) => self.cycle_argument(&arg),
            _ => None,
        };

        if completion.is_none() {
            return Ok(Response::Skip);
        }

        context.line_suffix = completion.unwrap();
        context.set_dirty_offset_from_line_suffix(0);
        Ok(Response::Ok)
    }

    fn cycle_command(&mut self, cmd: &String) -> Option<String> {
        let complete_cmd = match self.find_next(|app| app.command.starts_with(cmd)) {
            Some(app) => &app.command[cmd.len()..],
            None => return None,
        };

        if complete_cmd.is_empty() {
            return None;
        }
        Some(complete_cmd.to_string())
    }

    fn cycle_argument(&mut self, arg: &String) -> Option<String> {
        warn!("Autocomplete arg not implemented yet");
        None
    }

    fn find_next<F>(&mut self, mut predicate: F) -> Option<&Application>
    where
        F: FnMut(&Application) -> bool,
    {
        let length = self.applications.len();
        if length == 0 {
            return None;
        }

        for offset in 1..=length {
            let index = (self.current_index + offset) % length;
            let application = &self.applications[index];
            if predicate(application) {
                self.current_index = index;
                return Some(application);
            }
        }
        None
    }
}
