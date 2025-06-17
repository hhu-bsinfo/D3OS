use alloc::{format, string::String};
use naming::cwd;

use crate::{
    context::Context,
    event::event_handler::{Error, EventHandler, Response},
};

const INDICATOR: char = '>';

pub struct CommandLine {}

impl EventHandler for CommandLine {
    fn prepare(&mut self, context: &mut Context) -> Result<Response, Error> {
        context.line.reset();
        context.cursor_position = 0;
        self.set_prefix(context);

        Ok(Response::Ok)
    }

    fn simple_key(&mut self, context: &mut Context, key: char) -> Result<Response, Error> {
        match key {
            '\x08' => self.remove_before_cursor(context),
            '\x7F' => self.remove_at_cursor(context),
            _ => self.add_at_cursor(context, key),
        }
    }

    fn cursor_left(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.move_cursor_left(context)
    }

    fn cursor_right(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.move_cursor_right(context)
    }
}

impl CommandLine {
    pub const fn new() -> Self {
        Self {}
    }

    fn set_prefix(&mut self, context: &mut Context) -> Result<Response, Error> {
        let string = format!("{}{} ", cwd().unwrap_or(String::new()), INDICATOR);
        context.indicator.set(&string);
        Ok(Response::Ok)
    }

    fn remove_at_cursor(&mut self, context: &mut Context) -> Result<Response, Error> {
        if context.cursor_position >= context.line.len() {
            return Ok(Response::Ok);
        }

        context.line.remove(context.cursor_position);
        Ok(Response::Ok)
    }

    fn remove_before_cursor(&mut self, context: &mut Context) -> Result<Response, Error> {
        if context.cursor_position == 0 {
            return Ok(Response::Skip);
        }

        context.line.remove(context.cursor_position - 1);
        context.cursor_position -= 1;
        Ok(Response::Ok)
    }

    fn add_at_cursor(&mut self, context: &mut Context, ch: char) -> Result<Response, Error> {
        context.line.insert(context.cursor_position, ch);
        context.cursor_position = context.cursor_position + 1;
        Ok(Response::Ok)
    }

    fn move_cursor_right(&mut self, context: &mut Context) -> Result<Response, Error> {
        if context.cursor_position >= context.line.len() {
            return Ok(Response::Skip);
        }

        context.cursor_position = context.cursor_position + 1;
        Ok(Response::Ok)
    }

    fn move_cursor_left(&mut self, context: &mut Context) -> Result<Response, Error> {
        if context.cursor_position <= 0 {
            return Ok(Response::Skip);
        }

        context.cursor_position = context.cursor_position - 1;
        Ok(Response::Ok)
    }
}
