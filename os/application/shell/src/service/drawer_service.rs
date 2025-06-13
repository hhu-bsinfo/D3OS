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
        self.terminal_cursor_pos = 0;
        self.draw_at_dirty(context)
    }

    fn submit(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.draw_next_line(context)
    }

    fn auto_complete(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.draw_at_dirty(context)
    }

    fn cursor_left(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.draw_at_dirty(context)
    }

    fn cursor_right(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.draw_at_dirty(context)
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

    fn draw_next_line(&mut self, context: &mut Context) -> Result<Response, Error> {
        print!("{}\n", self.cursor_to_end(context));
        Ok(Response::Ok)
    }

    fn draw_at_dirty(&mut self, context: &mut Context) -> Result<Response, Error> {
        print!(
            "{}{}{}{}[38;2;100;100;100m{}[0m{}",
            self.cursor_to_dirty(context),
            self.clear_right_of_cursor(),
            self.dirty_indicator(context),
            self.dirty_line(context),
            self.dirty_suggestion(context),
            self.restore_cursor_position(context)
        );

        context.line_dirty_at = context.line.len();
        context.is_indicator_dirty = false;
        context.is_suggestion_dirty = false;

        Ok(Response::Ok)
    }

    fn draw_cursor(&mut self, context: &mut Context) -> Result<Response, Error> {
        print!("{}", self.restore_cursor_position(context));
        Ok(Response::Ok)
    }

    fn cursor_to_start(&mut self) -> String {
        let step = -(self.terminal_cursor_pos as isize);
        self.move_cursor_by(step)
    }

    fn cursor_to_end(&mut self, context: &mut Context) -> String {
        let step = self.terminal_cursor_pos as isize - context.total_line_len() as isize;
        self.move_cursor_by(step)
    }

    fn cursor_to_dirty(&mut self, context: &mut Context) -> String {
        let step = match context.is_indicator_dirty {
            true => -(self.terminal_cursor_pos as isize),
            false => {
                self.terminal_cursor_pos as isize
                    - context.indicator.len() as isize
                    - context.line_dirty_at as isize
            }
        };

        self.move_cursor_by(step)
    }

    fn restore_cursor_position(&mut self, context: &mut Context) -> String {
        let step = match context.is_autocompletion_active {
            true => self.terminal_cursor_pos as isize - context.total_line_len() as isize,
            false => {
                self.terminal_cursor_pos as isize
                    - context.cursor_position as isize
                    - context.indicator.len() as isize
            }
        };
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

    fn dirty_indicator(&mut self, context: &mut Context) -> String {
        if !context.is_indicator_dirty {
            return String::new();
        }

        self.terminal_cursor_pos += context.indicator.len();
        context.indicator.clone()
    }

    fn dirty_line(&mut self, context: &mut Context) -> String {
        let start_at = match context.is_indicator_dirty {
            true => 0,
            false => context.line_dirty_at,
        };

        let line = context.line[start_at..].to_string();
        self.terminal_cursor_pos += line.len();
        line
    }

    fn dirty_suggestion(&mut self, context: &mut Context) -> String {
        if !context.is_suggestion_dirty {
            return String::new();
        }

        self.terminal_cursor_pos += context.suggestion.len();
        context.suggestion.clone()
    }
}
