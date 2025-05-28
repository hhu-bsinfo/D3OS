use crate::context::Context;

use super::service::{Error, Response, Service};

pub struct CommandLineService {}

impl Service for CommandLineService {
    fn prepare(&mut self, context: &mut Context) -> Result<Response, Error> {
        context.line.clear();
        context.cursor_position = 0;
        context.dirty_offset = 0;
        Ok(Response::Ok)
    }

    fn simple_key(&mut self, context: &mut Context, key: char) -> Result<Response, Error> {
        match key {
            '\x08' => self.handle_backspace(context),
            '\x7F' => self.handle_del(context),
            _ => self.on_other_char(context, key),
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

    fn handle_del(&mut self, context: &mut Context) -> Result<Response, Error> {
        if context.cursor_position >= context.line.len() {
            return Ok(Response::Ok);
        }

        context.line.remove(context.cursor_position);
        context.set_dirty_offset_from_line(context.cursor_position);
        Ok(Response::Ok)
    }

    fn handle_backspace(&mut self, context: &mut Context) -> Result<Response, Error> {
        if context.cursor_position == 0 {
            return Ok(Response::Skip);
        }

        context.line.remove(context.cursor_position - 1);
        context.set_dirty_offset_from_line(context.cursor_position - 1);
        context.cursor_position -= 1;
        Ok(Response::Ok)
    }

    fn on_other_char(&mut self, context: &mut Context, ch: char) -> Result<Response, Error> {
        context.line.insert(context.cursor_position, ch);
        context.set_dirty_offset_from_line(context.cursor_position);
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
