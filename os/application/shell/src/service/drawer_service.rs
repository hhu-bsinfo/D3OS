use alloc::{
    format,
    string::{String, ToString},
};
use logger::info;
use terminal::{DecodedKey, print};

use crate::context::Context;

use super::service::{Service, ServiceError};

pub struct DrawerService {
    terminal_cursor_pos: usize,
}

impl Service for DrawerService {
    fn run(&mut self, event: DecodedKey, context: &mut Context) -> Result<(), ServiceError> {
        match event {
            DecodedKey::Unicode('\n') => self.on_enter(context),
            _ => self.on_other_key(context),
        }
    }
}

impl DrawerService {
    pub const fn new() -> Self {
        Self {
            terminal_cursor_pos: 0,
        }
    }

    fn on_enter(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        self.terminal_cursor_pos = context.line_prefix.len();
        print!("\n");
        Ok(())
    }

    fn on_other_key(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        print!(
            "{}{}{}{}",
            self.move_cursor_to_dirty_offset(context),
            self.remove_right_of_cursor(),
            self.line_from_dirty_line_offset(context),
            self.move_cursor_to_cursor_position(context)
        );

        Ok(())
    }

    fn move_cursor_to_dirty_offset(&mut self, context: &mut Context) -> String {
        let step = self.terminal_cursor_pos as isize - context.dirty_offset as isize;
        info!(
            "terminal_cursor({}) - dirty_offset({}) = {}",
            self.terminal_cursor_pos, context.dirty_offset, step
        );
        self.move_cursor(step)
    }

    fn move_cursor_to_cursor_position(&mut self, context: &mut Context) -> String {
        let step = self.terminal_cursor_pos as isize - context.cursor_position as isize;
        info!(
            "terminal_cursor({}) - cursor_pos({}) = {}",
            self.terminal_cursor_pos, context.cursor_position, step
        );
        self.move_cursor(step)
    }

    fn move_cursor(&mut self, step: isize) -> String {
        self.terminal_cursor_pos = (self.terminal_cursor_pos as isize - step) as usize;
        match step {
            0 => "".to_string(),
            offset if offset < 0 => format!("\x1b[{}C", offset.abs()),
            offset => format!("\x1b[{}D", offset),
        }
    }

    fn remove_right_of_cursor(&self) -> &'static str {
        "\x1b[0K"
    }

    fn line_from_dirty_line_offset(&mut self, context: &mut Context) -> String {
        let start_at = context.get_dirty_line_offset();
        let line: String = context.line[start_at..].iter().collect();
        self.terminal_cursor_pos += line.len();
        line
    }
}
