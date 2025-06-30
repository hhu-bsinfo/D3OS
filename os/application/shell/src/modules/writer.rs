use alloc::{
    format,
    string::{String, ToString},
};
use terminal::print;

use crate::{
    context::context::Context,
    event::event_handler::{Error, EventHandler, Response},
};

pub struct Writer {
    terminal_cursor_pos: usize,
}

impl EventHandler for Writer {
    fn on_prepare_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.terminal_cursor_pos = 0;
        self.draw_at_dirty(clx)
    }

    fn on_submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.draw_next_line(clx)
    }

    fn on_process_completed(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.draw_at_dirty(clx);
        self.recolor_indicator(clx) // TODO improve performance
    }
}

impl Writer {
    pub const fn new() -> Self {
        Self { terminal_cursor_pos: 0 }
    }

    fn draw_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        print!("{}\n", self.cursor_to_end(clx));
        Ok(Response::Ok)
    }

    fn draw_at_dirty(&mut self, clx: &mut Context) -> Result<Response, Error> {
        print!(
            "{}{}{}{}[38;2;100;100;100m{}[0m{}",
            self.cursor_to_dirty(clx),
            self.clear_right_of_cursor(),
            self.dirty_indicator(clx),
            self.dirty_line(clx),
            self.dirty_suggestion(clx),
            self.restore_cursor_position(clx)
        );

        clx.line.mark_clean();
        clx.indicator.mark_clean();
        clx.suggestion.mark_clean();

        Ok(Response::Ok)
    }

    fn recolor_indicator(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let (color_start, color_end) = if clx.tokens.is_error() {
            ("[38;2;255;0;0m", "[0m")
        } else if clx.tokens.is_incomplete() {
            ("[38;2;255;255;0m", "[0m")
        } else {
            ("", "")
        };

        print!(
            "{}{}{}{}{}{}",
            "\x1b[s",
            "\x1b[G",
            color_start,
            clx.indicator.get(),
            color_end,
            "\x1b[u",
        );

        Ok(Response::Ok)
    }

    fn draw_cursor(&mut self, clx: &mut Context) -> Result<Response, Error> {
        print!("{}", self.restore_cursor_position(clx));
        Ok(Response::Ok)
    }

    fn cursor_to_start(&mut self) -> String {
        let step = -(self.terminal_cursor_pos as isize);
        self.move_cursor_by(step)
    }

    fn cursor_to_end(&mut self, clx: &mut Context) -> String {
        let step = self.terminal_cursor_pos as isize - clx.total_line_len() as isize;
        self.move_cursor_by(step)
    }

    fn cursor_to_dirty(&mut self, clx: &mut Context) -> String {
        let step = match clx.indicator.is_dirty() {
            true => -(self.terminal_cursor_pos as isize),
            false => {
                self.terminal_cursor_pos as isize - clx.indicator.len() as isize - clx.line.get_dirty_index() as isize
            }
        };

        self.move_cursor_by(step)
    }

    fn restore_cursor_position(&mut self, clx: &mut Context) -> String {
        let step = match clx.suggestion.has_focus() {
            true => self.terminal_cursor_pos as isize - clx.total_line_len() as isize,
            false => {
                self.terminal_cursor_pos as isize - clx.line.get_cursor_pos() as isize - clx.indicator.len() as isize
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

    fn dirty_indicator(&mut self, clx: &mut Context) -> String {
        let indicator = match clx.indicator.is_dirty() {
            true => clx.indicator.get().clone(),
            false => String::new(),
        };

        self.terminal_cursor_pos += indicator.len();
        indicator
    }

    fn dirty_line(&mut self, clx: &mut Context) -> String {
        let line = match clx.indicator.is_dirty() {
            true => clx.line.get(),
            false => clx.line.get_dirty_part(),
        };

        self.terminal_cursor_pos += line.len();
        line.to_string()
    }

    fn dirty_suggestion(&mut self, clx: &mut Context) -> String {
        let line = match clx.suggestion.is_dirty() {
            true => clx.suggestion.get().clone(),
            false => String::new(),
        };

        self.terminal_cursor_pos += line.len();
        line
    }
}
