use core::cell::RefCell;

use alloc::{format, rc::Rc, string::String};
use naming::cwd;
use terminal::{DecodedKey, KeyCode};

use crate::{
    context::{indicator_context::IndicatorContext, line_context::LineContext},
    event::{
        event::Event,
        event_bus::EventBus,
        event_handler::{Error, EventHandler, Response},
    },
};

const INDICATOR: char = '>';

pub struct CommandLine {
    line_provider: Rc<RefCell<LineContext>>,
    indicator_provider: Rc<RefCell<IndicatorContext>>,
}

impl EventHandler for CommandLine {
    fn on_key_pressed(&mut self, event_bus: &mut EventBus, key: DecodedKey) -> Result<Response, Error> {
        let mut line_clx = self.line_provider.borrow_mut();

        match key {
            DecodedKey::RawKey(KeyCode::ArrowLeft) => Self::move_cursor_left(&mut line_clx, event_bus),
            DecodedKey::RawKey(KeyCode::ArrowRight) => Self::move_cursor_right(&mut line_clx, event_bus),
            DecodedKey::RawKey(KeyCode::Home) => Self::move_cursor_to_start(&mut line_clx),
            DecodedKey::RawKey(KeyCode::End) => Self::move_cursor_to_end(&mut line_clx),
            DecodedKey::RawKey(_) => Ok(Response::Skip),

            DecodedKey::Unicode('\t') | DecodedKey::Unicode('\x1B') => Ok(Response::Skip),

            DecodedKey::Unicode('\n') => Self::submit(event_bus),
            DecodedKey::Unicode('\x08') => Self::remove_before_cursor(&mut line_clx, event_bus),
            DecodedKey::Unicode('\x7F') => Self::remove_at_cursor(&mut line_clx, event_bus),
            DecodedKey::Unicode(ch) => Self::add_at_cursor(&mut line_clx, event_bus, ch),
        }
    }

    fn on_prepare_next_line(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        let mut line_clx = self.line_provider.borrow_mut();
        let mut indicator_clx = self.indicator_provider.borrow_mut();

        line_clx.reset();
        Self::set_prefix(&mut indicator_clx);
        Ok(Response::Ok)
    }
}

impl CommandLine {
    pub const fn new(
        line_provider: Rc<RefCell<LineContext>>,
        indicator_provider: Rc<RefCell<IndicatorContext>>,
    ) -> Self {
        Self {
            line_provider,
            indicator_provider,
        }
    }

    fn submit(event_bus: &mut EventBus) -> Result<Response, Error> {
        event_bus.trigger(Event::Submit);
        Ok(Response::Ok)
    }

    fn set_prefix(indicator_clx: &mut IndicatorContext) -> Result<Response, Error> {
        let string = format!("{}{} ", cwd().unwrap_or(String::new()), INDICATOR);
        indicator_clx.set(&string);
        Ok(Response::Ok)
    }

    fn remove_at_cursor(line_clx: &mut LineContext, event_bus: &mut EventBus) -> Result<Response, Error> {
        if line_clx.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        line_clx.remove(line_clx.get_cursor_pos());
        event_bus.trigger(Event::LineWritten);
        Ok(Response::Ok)
    }

    fn remove_before_cursor(line_clx: &mut LineContext, event_bus: &mut EventBus) -> Result<Response, Error> {
        if line_clx.is_cursor_at_start() {
            return Ok(Response::Skip);
        }

        line_clx.remove(line_clx.get_cursor_pos() - 1);
        line_clx.move_cursor_left(1);
        event_bus.trigger(Event::LineWritten);
        Ok(Response::Ok)
    }

    fn add_at_cursor(line_clx: &mut LineContext, event_bus: &mut EventBus, ch: char) -> Result<Response, Error> {
        line_clx.insert(line_clx.get_cursor_pos(), ch);
        line_clx.move_cursor_right(1);
        event_bus.trigger(Event::LineWritten);
        Ok(Response::Ok)
    }

    fn move_cursor_to_start(line_clx: &mut LineContext) -> Result<Response, Error> {
        line_clx.set_cursor_pos(0);
        Ok(Response::Ok)
    }

    fn move_cursor_to_end(line_clx: &mut LineContext) -> Result<Response, Error> {
        let end_pos = line_clx.len();
        line_clx.set_cursor_pos(end_pos);
        Ok(Response::Ok)
    }

    fn move_cursor_right(line_clx: &mut LineContext, event_bus: &mut EventBus) -> Result<Response, Error> {
        if line_clx.is_cursor_at_end() {
            return Ok(Response::Skip);
        }

        line_clx.move_cursor_right(1);
        event_bus.trigger(Event::CursorMoved(1));
        Ok(Response::Ok)
    }

    fn move_cursor_left(line_clx: &mut LineContext, event_bus: &mut EventBus) -> Result<Response, Error> {
        if line_clx.is_cursor_at_start() {
            return Ok(Response::Skip);
        }

        line_clx.move_cursor_left(1);
        event_bus.trigger(Event::CursorMoved(-1));
        Ok(Response::Ok)
    }
}
