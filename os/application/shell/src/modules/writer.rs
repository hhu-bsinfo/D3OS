use core::cell::RefCell;

use alloc::{
    format,
    rc::Rc,
    string::{String, ToString},
};
use logger::warn;
use terminal::print;

use crate::{
    context::context::Context,
    event::event_handler::{Error, EventHandler, Response},
    modules::parser::token::{ArgumentKind, Token, TokenKind, TokenStatus},
    sub_modules::theme_provider::ThemeProvider,
};

pub struct Writer {
    theme_provider: Rc<RefCell<ThemeProvider>>,
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
    pub const fn new(theme_provider: Rc<RefCell<ThemeProvider>>) -> Self {
        Self {
            theme_provider,
            terminal_cursor_pos: 0,
        }
    }

    fn write_indicator(&mut self, clx: &mut Context) -> Result<Response, Error> {
        print!(
            "{}{}\x1b[0m",
            self.indicator_color(&TokenStatus::Valid),
            clx.indicator.get()
        );
        self.terminal_cursor_pos += clx.indicator.len();
        Ok(Response::Ok)
    }

    fn write_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        print!("{}\n", self.cursor_to_end(clx));
        Ok(Response::Ok)
    }

    fn write_at_dirty(&mut self, clx: &mut Context) -> Result<Response, Error> {
        warn!("last token: {:?}", clx.tokens.last());
        warn!("dirty at: {}", clx.line.get_dirty_index());
        print!(
            "{}{}{}{}{}{}",
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
        format!(
            "{}{}{}{}\x1b[0m{}",
            Self::save_cursor_pos(),
            Self::cursor_to_start(),
            self.indicator_color(clx.tokens.status()),
            clx.indicator.get(),
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
            let dirty_content = token.as_str_at_line_index(clx.line.get_dirty_index());
            let color = self.token_color(token);
            formatted_tokens.push_str(color);
            formatted_tokens.push_str(dirty_content);
            formatted_tokens.push_str("\x1b[0m");
            self.terminal_cursor_pos += dirty_content.len();
        }
        formatted_tokens
    }

    fn dirty_suggestion(&mut self, clx: &mut Context) -> String {
        if !clx.suggestion.is_dirty() {
            return String::new();
        }
        let theme = self.theme_provider.borrow().get();
        let line = clx.suggestion.get();
        self.terminal_cursor_pos += line.len();
        format!("{}{}\x1b[0m", theme.suggestion, line)
    }

    fn indicator_color(&self, status: &TokenStatus) -> &'static str {
        let theme = self.theme_provider.borrow().get();
        match *status {
            TokenStatus::Valid => theme.indicator,
            TokenStatus::Incomplete => theme.indicator_warning,
            TokenStatus::Error(_) => theme.indicator_error,
        }
    }

    fn token_color(&self, token: &Token) -> &'static str {
        let theme = self.theme_provider.borrow().get();
        if token.clx().in_quote.is_some() {
            return theme.in_quote;
        }
        match token.kind() {
            TokenKind::Command => theme.cmd,
            TokenKind::Argument => match token.clx().arg_kind {
                ArgumentKind::None => "",
                ArgumentKind::Generic | ArgumentKind::ShortOrLongFlag => theme.generic_arg,
                ArgumentKind::ShortFlag => theme.short_flag_arg,
                ArgumentKind::ShortFlagValue => theme.short_flag_value_arg,
                ArgumentKind::LongFlag => theme.long_flag_arg,
            },
            TokenKind::Blank => "",
            TokenKind::QuoteStart => theme.quote_start,
            TokenKind::QuoteEnd => theme.quote_end,
            TokenKind::Pipe => theme.pipe,
            TokenKind::Separator => theme.separator,
            TokenKind::Background => theme.background,
            TokenKind::And => theme.logical_and,
            TokenKind::Or => theme.logical_or,
            TokenKind::RedirectInTruncate => theme.redirection_in_truncate,
            TokenKind::RedirectInAppend => theme.redirection_in_append,
            TokenKind::RedirectOutTruncate => theme.redirection_out_truncate,
            TokenKind::RedirectOutAppend => theme.redirection_out_append,
            TokenKind::File => theme.file,
        }
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
