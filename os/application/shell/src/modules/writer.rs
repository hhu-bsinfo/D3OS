use alloc::{
    format,
    string::{String, ToString},
};
use terminal::print;

use crate::{
    context::context::Context,
    event::event_handler::{Error, EventHandler, Response},
    modules::parser::token::{ArgumentKind, TokenKind, TokenStatus},
};

pub struct Writer {
    terminal_cursor_pos: usize,
}

impl EventHandler for Writer {
    fn on_prepare_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.terminal_cursor_pos = 0;
        self.write_indicator(clx)
    }

    fn on_submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.write_next_line(clx)
    }

    fn on_process_completed(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.write_at_dirty(clx)
    }
}

impl Writer {
    pub const fn new() -> Self {
        Self { terminal_cursor_pos: 0 }
    }

    fn write_indicator(&mut self, clx: &mut Context) -> Result<Response, Error> {
        print!("{}", clx.indicator.get());
        self.terminal_cursor_pos += clx.indicator.len();
        Ok(Response::Ok)
    }

    fn write_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        print!("{}\n", self.cursor_to_end(clx));
        Ok(Response::Ok)
    }

    fn write_at_dirty(&mut self, clx: &mut Context) -> Result<Response, Error> {
        print!(
            "{}{}{}{}[38;2;100;100;100m{}[0m{}",
            self.dirty_status_indicator(clx),
            self.cursor_to_dirty_line(clx),
            Self::clear_right_of_cursor(),
            self.dirty_tokens(clx),
            self.dirty_suggestion(clx),
            self.restore_cursor_position(clx)
        );

        clx.line.mark_clean();
        clx.tokens.mark_status_clean();
        clx.suggestion.mark_clean();

        Ok(Response::Ok)
    }

    fn dirty_status_indicator(&mut self, clx: &mut Context) -> String {
        if !clx.tokens.is_status_dirty() {
            return String::new();
        }
        let (color_start, color_end) = match clx.tokens.status() {
            TokenStatus::Valid => ("", ""),
            TokenStatus::Incomplete => ("[38;2;255;255;0m", "[0m"),
            TokenStatus::Error(_) => ("[38;2;255;0;0m", "[0m"),
        };
        format!(
            "{}{}{}{}{}{}",
            Self::save_cursor_pos(),
            Self::cursor_to_start(),
            color_start,
            clx.indicator.get(),
            color_end,
            Self::restore_cursor_pos(),
        )
    }

    fn cursor_to_end(&mut self, clx: &mut Context) -> String {
        let step = self.terminal_cursor_pos as isize - clx.total_line_len() as isize;
        self.move_cursor_by(step)
    }

    fn cursor_to_dirty_line(&mut self, clx: &mut Context) -> String {
        let offset = clx.indicator.len() + clx.line.get_dirty_index();
        let step = self.terminal_cursor_pos as isize - offset as isize;
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

    /// Replaced with dirty_tokens
    fn dirty_line(&mut self, clx: &mut Context) -> String {
        let line = clx.line.get_dirty_part();
        self.terminal_cursor_pos += line.len();
        line.to_string()
    }

    fn dirty_tokens(&mut self, clx: &mut Context) -> String {
        let mut formatted_tokens = String::new();
        for token in clx.tokens.slice_at_line_index(clx.line.get_dirty_index()) {
            let color = if token.clx().in_quote.is_some() {
                "\x1b[38;2;0;255;0m" // lime
            } else {
                match token.kind() {
                    TokenKind::Command => "\x1b[38;2;255;215;0m", // gold
                    TokenKind::Argument => match token.clx().arg_kind {
                        ArgumentKind::None => "\x1b[38;2;192;192;255m",            // pale blue
                        ArgumentKind::ShortOrLongFlag => "\x1b[38;2;160;160;255m", // light blue
                        ArgumentKind::Generic => "\x1b[38;2;128;128;255m",         // medium blue
                        ArgumentKind::ShortFlag => "\x1b[38;2;64;64;255m",         // blue
                        ArgumentKind::ShortFlagValue => "\x1b[38;2;0;0;255m",      // vivid blue
                        ArgumentKind::LongFlag => "\x1b[38;2;0;0;200m",            // deep blue
                    },
                    TokenKind::Blank => "\x1b[38;2;128;128;128m",  // gray
                    TokenKind::QuoteStart => "\x1b[38;2;0;255;0m", // lime
                    TokenKind::QuoteEnd => "\x1b[38;2;0;255;0m",   // lime
                    TokenKind::Pipe => "\x1b[38;2;255;0;0m",       // red
                    TokenKind::Separator => "\x1b[38;2;255;0;0m",  // red
                    TokenKind::Background => "\x1b[38;2;210;180;140m", // tan
                    TokenKind::And => "\x1b[38;2;255;165;0m",      // orange
                    TokenKind::Or => "\x1b[38;2;255;165;0m",       // orange
                    TokenKind::RedirectInTruncate => "\x1b[38;2;255;0;0m", // red
                    TokenKind::RedirectInAppend => "\x1b[38;2;255;0;0m", // red
                    TokenKind::RedirectOutTruncate => "\x1b[38;2;255;0;0m", // red
                    TokenKind::RedirectOutAppend => "\x1b[38;2;255;0;0m", // red
                    TokenKind::File => "\x1b[38;2;128;0;128m",     // purple
                }
            };
            let dirty_content = token.as_str_at_line_index(clx.line.get_dirty_index());
            formatted_tokens.push_str(color);
            formatted_tokens.push_str(dirty_content);
            formatted_tokens.push_str("\x1b[0m");
            self.terminal_cursor_pos += dirty_content.len();
        }
        formatted_tokens
    }

    fn dirty_suggestion(&mut self, clx: &mut Context) -> String {
        let line = match clx.suggestion.is_dirty() {
            true => clx.suggestion.get().clone(),
            false => String::new(),
        };

        self.terminal_cursor_pos += line.len();
        line
    }

    fn clear_right_of_cursor() -> &'static str {
        "\x1b[0K"
    }

    fn cursor_to_start() -> &'static str {
        "\x1b[G"
    }

    fn save_cursor_pos() -> &'static str {
        "\x1b[s"
    }

    fn restore_cursor_pos() -> &'static str {
        "\x1b[u"
    }
}
