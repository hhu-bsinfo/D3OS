use alloc::{format, string::String};
use naming::cwd;
use terminal::{DecodedKey, KeyCode};

use crate::{
    context::context::Context,
    event::{
        event::Event,
        event_handler::{Error, EventHandler, Response},
    },
};

const INDICATOR: char = '>';

pub struct CommandLine {}

impl EventHandler for CommandLine {
    fn on_key_pressed(&mut self, clx: &mut Context, key: DecodedKey) -> Result<Response, Error> {
        match key {
            DecodedKey::RawKey(KeyCode::ArrowLeft) => self.move_cursor_left(clx),
            DecodedKey::RawKey(KeyCode::ArrowRight) => self.move_cursor_right(clx),
            DecodedKey::RawKey(_) => Ok(Response::Skip),

            DecodedKey::Unicode('\t') | DecodedKey::Unicode('\x1B') => Ok(Response::Skip),

            DecodedKey::Unicode('\n') => self.submit(clx),
            DecodedKey::Unicode('\x08') => self.remove_before_cursor(clx),
            DecodedKey::Unicode('\x7F') => self.remove_at_cursor(clx),
            DecodedKey::Unicode(ch) => self.add_at_cursor(clx, ch),
        }
    }

    fn on_prepare_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        clx.line.reset();
        self.set_prefix(clx);
        Ok(Response::Ok)
    }
}

impl CommandLine {
    pub const fn new() -> Self {
        Self {}
    }

    fn submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        clx.events.trigger(Event::Submit);
        Ok(Response::Ok)
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
        clx.events.trigger(Event::LineWritten);
        Ok(Response::Ok)
    }

    fn remove_before_cursor(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if clx.line.is_cursor_at_start() {
            return Ok(Response::Skip);
        }

        clx.line.remove(clx.line.get_cursor_pos() - 1);
        clx.line.move_cursor_left(1);
        clx.events.trigger(Event::LineWritten);
        Ok(Response::Ok)
    }

    fn add_at_cursor(&mut self, clx: &mut Context, ch: char) -> Result<Response, Error> {
        clx.line.insert(clx.line.get_cursor_pos(), ch);
        clx.line.move_cursor_right(1);
        clx.events.trigger(Event::LineWritten);
        Ok(Response::Ok)
    }

    fn move_cursor_right(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if clx.line.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        clx.line.move_cursor_right(1);
        clx.events.trigger(Event::CursorMoved(1));
        Ok(Response::Ok)
    }

    fn move_cursor_left(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if clx.line.is_cursor_at_start() {
            return Ok(Response::Skip);
        }

        clx.line.move_cursor_left(1);
        clx.events.trigger(Event::CursorMoved(-1));
        Ok(Response::Ok)
    }
}
