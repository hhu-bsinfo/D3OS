use alloc::{format, string::String};
use naming::cwd;

use crate::context::Context;

use super::service::{Error, Response, Service};

const INDICATOR: char = '>';

pub struct CommandLineService {}

impl Service for CommandLineService {
    fn prepare(&mut self, context: &mut Context) -> Result<Response, Error> {
        context.line.clear();
        context.cursor_position = 0;
        context.line_dirty_at = 0;
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

impl CommandLineService {
    pub const fn new() -> Self {
        Self {}
    }

    fn set_prefix(&mut self, context: &mut Context) -> Result<Response, Error> {
        context.indicator = format!("{}{} ", cwd().unwrap_or(String::new()), INDICATOR);
        context.is_indicator_dirty = true;
        Ok(Response::Ok)
    }

    fn remove_at_cursor(&mut self, context: &mut Context) -> Result<Response, Error> {
        if context.cursor_position >= context.line.len() {
            return Ok(Response::Ok);
        }

        context.line.remove(context.cursor_position);
        context.set_dirty_line_index(context.cursor_position);
        Ok(Response::Ok)
    }

    fn remove_before_cursor(&mut self, context: &mut Context) -> Result<Response, Error> {
        if context.cursor_position == 0 {
            return Ok(Response::Skip);
        }

        context.line.remove(context.cursor_position - 1);
        context.set_dirty_line_index(context.cursor_position - 1);
        context.cursor_position -= 1;
        Ok(Response::Ok)
    }

    fn add_at_cursor(&mut self, context: &mut Context, ch: char) -> Result<Response, Error> {
        context.line.insert(context.cursor_position, ch);
        context.set_dirty_line_index(context.cursor_position);
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
