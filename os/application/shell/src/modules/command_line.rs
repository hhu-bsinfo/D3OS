use alloc::{format, string::String};
use naming::cwd;

use crate::{
    context::context::Context,
    event::event_handler::{Error, EventHandler, Response},
};

const INDICATOR: char = '>';

pub struct CommandLine {}

impl EventHandler for CommandLine {
    fn prepare(&mut self, clx: &mut Context) -> Result<Response, Error> {
        clx.line.reset();
        self.set_prefix(clx);

        Ok(Response::Ok)
    }

    fn simple_key(&mut self, clx: &mut Context, key: char) -> Result<Response, Error> {
        match key {
            '\x08' => self.remove_before_cursor(clx),
            '\x7F' => self.remove_at_cursor(clx),
            _ => self.add_at_cursor(clx, key),
        }
    }

    fn cursor_left(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.move_cursor_left(clx)
    }

    fn cursor_right(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.move_cursor_right(clx)
    }
}

impl CommandLine {
    pub const fn new() -> Self {
        Self {}
    }

    fn set_prefix(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let string = format!("{}{} ", cwd().unwrap_or(String::new()), INDICATOR);
        clx.indicator.set(&string);
        Ok(Response::Ok)
    }

    fn remove_at_cursor(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if clx.line.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        clx.line.remove(clx.line.get_cursor_pos());
        Ok(Response::Ok)
    }

    fn remove_before_cursor(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if clx.line.is_cursor_at_start() {
            return Ok(Response::Skip);
        }

        clx.line.remove(clx.line.get_cursor_pos() - 1);
        clx.line.move_cursor_left(1);
        Ok(Response::Ok)
    }

    fn add_at_cursor(&mut self, clx: &mut Context, ch: char) -> Result<Response, Error> {
        clx.line.insert(clx.line.get_cursor_pos(), ch);
        clx.line.move_cursor_right(1);
        Ok(Response::Ok)
    }

    fn move_cursor_right(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if clx.line.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        clx.line.move_cursor_right(1);
        Ok(Response::Ok)
    }

    fn move_cursor_left(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if clx.line.is_cursor_at_start() {
            return Ok(Response::Skip);
        }

        clx.line.move_cursor_left(1);
        Ok(Response::Ok)
    }
}
