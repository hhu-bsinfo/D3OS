use core::cell::RefCell;

use alloc::{
    format,
    rc::Rc,
    string::{String, ToString},
};
use terminal::print;

use crate::{
    context::{
        indicator_context::IndicatorContext, line_context::LineContext, suggestion_context::SuggestionContext,
        theme_context::ThemeContext, tokens_context::TokensContext,
    },
    event::{
        event::Event,
        event_bus::EventBus,
        event_handler::{Error, EventHandler, Response},
    },
    token::token::{Token, TokenKind, TokenStatus},
};

pub struct Writer {
    line_provider: Rc<RefCell<LineContext>>,
    tokens_provider: Rc<RefCell<TokensContext>>,
    indicator_provider: Rc<RefCell<IndicatorContext>>,
    suggestion_provider: Rc<RefCell<SuggestionContext>>,
    theme_provider: Rc<RefCell<ThemeContext>>,

    terminal_cursor_pos: usize,
}

impl EventHandler for Writer {
    fn on_prepare_next_line(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.terminal_cursor_pos = 0;
        self.write_indicator()
    }

    fn on_submit(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.write_next_line()
    }

    fn on_process_completed(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.write_at_dirty()
    }

    fn on_process_failed(&mut self, event_bus: &mut EventBus, error: &Error) -> Result<Response, Error> {
        self.write_error(event_bus, error)
    }
}

impl Writer {
    pub const fn new(
        line_provider: Rc<RefCell<LineContext>>,
        tokens_provider: Rc<RefCell<TokensContext>>,
        indicator_provider: Rc<RefCell<IndicatorContext>>,
        suggestion_provider: Rc<RefCell<SuggestionContext>>,
        theme_provider: Rc<RefCell<ThemeContext>>,
    ) -> Self {
        Self {
            line_provider,
            tokens_provider,
            indicator_provider,
            suggestion_provider,
            theme_provider,
            terminal_cursor_pos: 0,
        }
    }

    fn write_indicator(&mut self) -> Result<Response, Error> {
        let indicator_clx = self.indicator_provider.borrow();
        print!(
            "{}{}\x1b[0m",
            self.indicator_color(&TokenStatus::Valid),
            indicator_clx.get()
        );
        self.terminal_cursor_pos += indicator_clx.len();
        Ok(Response::Ok)
    }

    fn write_next_line(&mut self) -> Result<Response, Error> {
        print!("{}\n", self.cursor_to_end());
        Ok(Response::Ok)
    }

    fn write_at_dirty(&mut self) -> Result<Response, Error> {
        print!(
            "{}{}{}{}{}{}",
            self.dirty_status_indicator(),
            self.cursor_to_dirty_line(),
            Self::clear_right_of_cursor(),
            self.dirty_tokens(),
            self.dirty_suggestion(),
            self.restore_cursor_position()
        );

        self.line_provider.borrow_mut().mark_clean();
        self.tokens_provider.borrow_mut().mark_status_clean();
        self.suggestion_provider.borrow_mut().mark_clean();

        Ok(Response::Ok)
    }

    fn write_error(&mut self, event_bus: &mut EventBus, error: &Error) -> Result<Response, Error> {
        let theme = self.theme_provider.borrow().get_current();
        let line_break = if error.is_in_execution { "" } else { "\n" };
        print!(
            "{}{}{}\x1b[0m\n{}{}\x1b[0m{}",
            line_break,
            theme.error_msg,
            error.message,
            theme.error_hint,
            error.hint.as_deref().unwrap_or(""),
            line_break
        );
        event_bus.trigger(Event::PrepareNewLine);
        Ok(Response::Ok)
    }

    fn dirty_status_indicator(&mut self) -> String {
        let tokens_clx = self.tokens_provider.borrow();
        let indicator_clx = self.indicator_provider.borrow();

        if !tokens_clx.is_status_dirty() {
            return String::new();
        }
        format!(
            "{}{}{}{}\x1b[0m{}",
            Self::save_cursor_pos(),
            Self::cursor_to_start(),
            self.indicator_color(&tokens_clx.status()),
            indicator_clx.get(),
            Self::restore_cursor_pos(),
        )
    }

    fn cursor_to_end(&mut self) -> String {
        let step = self.terminal_cursor_pos as isize - self.total_line_len() as isize;
        self.move_cursor_by(step)
    }

    fn cursor_to_dirty_line(&mut self) -> String {
        let offset = self.indicator_provider.borrow().len() + self.line_provider.borrow().get_dirty_index();
        let step = self.terminal_cursor_pos as isize - offset as isize;
        self.move_cursor_by(step)
    }

    fn restore_cursor_position(&mut self) -> String {
        let step = {
            let line_clx = self.line_provider.borrow();
            let suggestion_clx = self.suggestion_provider.borrow();
            let indicator_clx = self.indicator_provider.borrow();

            match suggestion_clx.has_focus() {
                true => self.terminal_cursor_pos as isize - self.total_line_len() as isize,
                false => {
                    self.terminal_cursor_pos as isize
                        - line_clx.get_cursor_pos() as isize
                        - indicator_clx.len() as isize
                }
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

    fn dirty_tokens(&mut self) -> String {
        let line_clx = self.line_provider.borrow();
        let tokens_clx = self.tokens_provider.borrow();
        let mut formatted_tokens = String::new();

        for token in tokens_clx.slice_at_line_index(line_clx.get_dirty_index()) {
            let dirty_content = token.as_str_at_line_pos(line_clx.get_dirty_index());
            let color = self.token_color(token);
            formatted_tokens.push_str(color);
            formatted_tokens.push_str(dirty_content);
            formatted_tokens.push_str("\x1b[0m");
            self.terminal_cursor_pos += dirty_content.len();
        }
        formatted_tokens
    }

    fn dirty_suggestion(&mut self) -> String {
        let suggestion_clx = self.suggestion_provider.borrow();
        if !suggestion_clx.is_dirty() {
            return String::new();
        }
        let theme = self.theme_provider.borrow().get_current();
        let line = suggestion_clx.get();
        self.terminal_cursor_pos += line.len();
        format!("{}{}\x1b[0m", theme.suggestion, line)
    }

    fn indicator_color(&self, status: &TokenStatus) -> &'static str {
        let theme = self.theme_provider.borrow().get_current();
        match *status {
            TokenStatus::Valid => theme.indicator,
            TokenStatus::Incomplete(_) => theme.indicator_warning,
            TokenStatus::Error(_) => theme.indicator_error,
        }
    }

    fn token_color(&self, token: &Token) -> &'static str {
        let theme = self.theme_provider.borrow().get_current();
        if token.clx().in_quote.is_some() && !matches!(token.kind(), TokenKind::QuoteStart | TokenKind::QuoteEnd) {
            return theme.in_quote;
        }
        match token.kind() {
            TokenKind::Command => theme.cmd,
            TokenKind::Argument => theme.arg,
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

    fn total_line_len(&self) -> usize {
        self.indicator_provider.borrow().len()
            + self.line_provider.borrow().len()
            + self.suggestion_provider.borrow().len()
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
