use terminal::{DecodedKey, KeyCode};

use crate::context::Context;

use super::service::{Service, ServiceError};

pub struct CommandLineService {}

impl Service for CommandLineService {
    fn run(&mut self, event: DecodedKey, context: &mut Context) -> Result<(), ServiceError> {
        match event {
            DecodedKey::Unicode('\n') => self.on_enter(context),
            DecodedKey::Unicode('\x08') => self.on_backspace(context),
            DecodedKey::Unicode('\x7F') => self.on_del(context),
            DecodedKey::Unicode(ch) => self.on_other_char(context, ch),
            DecodedKey::RawKey(KeyCode::ArrowLeft) => self.on_arrow_left(context),
            DecodedKey::RawKey(KeyCode::ArrowRight) => self.on_arrow_right(context),
            _ => Ok(()),
        }
    }
}

impl CommandLineService {
    pub const fn new() -> Self {
        Self {}
    }

    fn on_enter(&mut self, _context: &mut Context) -> Result<(), ServiceError> {
        Ok(())
    }

    fn on_del(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        if context.cursor_position >= context.line.len() {
            return Ok(());
        }

        context.line.remove(context.cursor_position);
        context.set_dirty_offset_from_line(context.cursor_position);
        Ok(())
    }

    fn on_backspace(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        if context.cursor_position == 0 {
            return Ok(());
        }

        context.line.remove(context.cursor_position - 1);
        context.set_dirty_offset_from_line(context.cursor_position - 1);
        context.cursor_position -= 1;
        Ok(())
    }

    fn on_other_char(&mut self, context: &mut Context, ch: char) -> Result<(), ServiceError> {
        context.line.insert(context.cursor_position, ch);
        context.set_dirty_offset_from_line(context.cursor_position);
        context.cursor_position = context.cursor_position + 1;
        Ok(())
    }

    fn on_arrow_right(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        if context.cursor_position >= context.line.len() {
            return Ok(());
        }

        context.cursor_position = context.cursor_position + 1;
        Ok(())
    }

    fn on_arrow_left(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        if context.cursor_position <= 0 {
            return Ok(());
        }

        context.cursor_position = context.cursor_position - 1;
        Ok(())
    }
}
