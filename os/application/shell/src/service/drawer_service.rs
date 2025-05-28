use alloc::{
    format,
    string::{String, ToString},
};
use terminal::print;

use crate::context::Context;

use super::service::{Error, Response, Service};

pub struct DrawerService {
    terminal_cursor_pos: usize,
}

impl Service for DrawerService {
    fn prepare(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.terminal_cursor_pos = context.line_prefix.len();
        Ok(Response::Ok)
    }

    fn submit(&mut self, _context: &mut Context) -> Result<Response, Error> {
        self.draw_next_line()
    }

    fn auto_complete(&mut self, _context: &mut Context) -> Result<Response, Error> {
        // TODO
        Ok(Response::Skip)
    }

    fn cursor_left(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.restore_cursor_position(context);
        Ok(Response::Ok)
    }

    fn cursor_right(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.restore_cursor_position(context);
        Ok(Response::Ok)
    }

    fn history_up(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.draw_at_dirty(context)
    }

    fn history_down(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.draw_at_dirty(context)
    }

    fn simple_key(&mut self, context: &mut Context, _key: char) -> Result<Response, Error> {
        self.draw_at_dirty(context)
    }
}

impl DrawerService {
    pub const fn new() -> Self {
        Self {
            terminal_cursor_pos: 0,
        }
    }

    fn draw_next_line(&mut self) -> Result<Response, Error> {
        self.terminal_cursor_pos = 0;
        print!("\n");
        Ok(Response::Ok)
    }

    fn draw_at_dirty(&mut self, context: &mut Context) -> Result<Response, Error> {
        print!(
            "{}{}{}{}",
            self.cursor_to_dirty(context),
            self.clear_right_of_cursor(),
            self.line_after_cursor(context),
            self.restore_cursor_position(context)
        );
        context.dirty_offset = context.total_line_len();
        Ok(Response::Ok)
    }

    fn cursor_to_dirty(&mut self, context: &mut Context) -> String {
        let step = self.terminal_cursor_pos as isize - context.dirty_offset as isize;
        self.move_cursor_by(step)
    }

    fn restore_cursor_position(&mut self, context: &mut Context) -> String {
        let step = self.terminal_cursor_pos as isize - context.cursor_position as isize;
        self.move_cursor_by(step)
    }

    fn move_cursor_by(&mut self, step: isize) -> String {
        self.terminal_cursor_pos = (self.terminal_cursor_pos as isize - step) as usize;
        match step {
            0 => "".to_string(),
            offset if offset < 0 => format!("\x1b[{}C", offset.abs()),
            offset => format!("\x1b[{}D", offset),
        }
    }

    fn clear_right_of_cursor(&self) -> &'static str {
        "\x1b[0K"
    }

    fn line_after_cursor(&mut self, context: &mut Context) -> String {
        let start_at = context.get_dirty_line_offset();
        let line: String = context.line[start_at..].iter().collect();
        self.terminal_cursor_pos += line.len();
        line
    }
}
