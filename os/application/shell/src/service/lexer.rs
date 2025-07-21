use alloc::string::{String, ToString};

use crate::{
    context::{
        alias_context::AliasContext, context::ContextProvider, line_context::LineContext, tokens_context::TokensContext,
    },
    event::{
        event::Event,
        event_bus::EventBus,
        event_handler::{Error, EventHandler, Response},
    },
    token::token::TokenKind,
};

pub struct LexerService {
    line_provider: ContextProvider<LineContext>,
    tokens_provider: ContextProvider<TokensContext>,
    alias_provider: ContextProvider<AliasContext>,
}

impl EventHandler for LexerService {
    fn on_prepare_next_line(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.tokens_provider.borrow_mut().reset();
        Ok(Response::Ok)
    }

    fn on_submit(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.retokenize_with_alias()
    }

    fn on_line_written(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        self.tokenize_from_dirty(event_bus)
    }

    fn on_history_restored(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        self.tokenize_from_dirty(event_bus)
    }
}

impl LexerService {
    pub const fn new(
        line_provider: ContextProvider<LineContext>,
        tokens_provider: ContextProvider<TokensContext>,
        alias_provider: ContextProvider<AliasContext>,
    ) -> Self {
        Self {
            line_provider,
            tokens_provider,
            alias_provider,
        }
    }

    fn tokenize_from_dirty(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        let mut line_clx = self.line_provider.borrow_mut();
        let mut tokens_clx = self.tokens_provider.borrow_mut();

        let dirty_index = line_clx.get_dirty_index();
        let detokenize_res = match Self::detokenize_to(&mut line_clx, &mut tokens_clx, dirty_index) {
            Ok(res) => res,
            Err(err) => return Err(err),
        };
        let tokenize_res = match Self::tokenize_from(&mut line_clx, &mut tokens_clx, dirty_index) {
            Ok(res) => res,
            Err(err) => return Err(err),
        };

        if detokenize_res == Response::Skip && tokenize_res == Response::Skip {
            return Ok(Response::Skip);
        }

        event_bus.trigger(Event::TokensWritten);
        Ok(Response::Ok)
    }

    fn detokenize_to(
        line_clx: &mut LineContext,
        tokens_clx: &mut TokensContext,
        index: usize,
    ) -> Result<Response, Error> {
        let total_len = tokens_clx.total_len();

        if total_len <= index {
            return Ok(Response::Skip);
        }

        let n = total_len - index;
        for _ in 0..n {
            Self::remove(line_clx, tokens_clx);
        }

        Ok(Response::Ok)
    }

    fn tokenize_from(
        line_clx: &mut LineContext,
        tokens_clx: &mut TokensContext,
        index: usize,
    ) -> Result<Response, Error> {
        if index >= line_clx.len() {
            return Ok(Response::Skip);
        }
        let dirty_line = line_clx.get()[index..].to_string();
        for ch in dirty_line.chars() {
            Self::add(line_clx, tokens_clx, ch);
        }
        Ok(Response::Ok)
    }

    fn retokenize_with_alias(&mut self) -> Result<Response, Error> {
        let mut tokens_clx = self.tokens_provider.borrow_mut();
        let mut line_clx = self.line_provider.borrow_mut();
        let alias_clx = self.alias_provider.borrow();

        let alias_line: String = tokens_clx
            .get()
            .iter()
            .map(|token| {
                if token.is_ambiguous() && token.clx().in_quote.is_none() {
                    alias_clx.get(token.as_str()).unwrap_or(token.as_str())
                } else {
                    token.as_str()
                }
            })
            .collect();

        tokens_clx.reset();

        for ch in alias_line.chars() {
            Self::add(&mut line_clx, &mut tokens_clx, ch);
        }

        Ok(Response::Ok)
    }

    fn add(line_clx: &mut LineContext, tokens_clx: &mut TokensContext, ch: char) {
        if tokens_clx
            .last()
            .is_some_and(|token| token.clx().in_quote.is_some_and(|quote| quote != ch))
        {
            Self::add_ambiguous(tokens_clx, ch);
            return;
        }

        match ch {
            // Job control
            ';' => Self::add_separator(tokens_clx, ch),
            '&' => Self::add_background_or_logical_and(line_clx, tokens_clx, ch),
            '|' => Self::add_pipe_or_logical_or(line_clx, tokens_clx, ch),
            // Redirection
            '>' => Self::add_redirect_out_append_or_truncate(line_clx, tokens_clx, ch),
            '<' => Self::add_redirect_in_file(tokens_clx, ch),
            // Quotes
            '\"' | '\'' => Self::add_quote(tokens_clx, ch),
            // Other
            ' ' | '\t' => Self::add_blank(tokens_clx, ch),
            ch => Self::add_ambiguous(tokens_clx, ch),
        }
    }

    fn remove(line_clx: &mut LineContext, tokens_clx: &mut TokensContext) {
        let Some(last_token) = tokens_clx.last() else {
            return;
        };

        let (kind, content) = match *last_token.kind() {
            TokenKind::And => (TokenKind::Background, "&".to_string()),
            TokenKind::Or => (TokenKind::Pipe, "|".to_string()),
            TokenKind::RedirectOutAppend => (TokenKind::RedirectOutTruncate, ">".to_string()),
            _ => {
                if tokens_clx.pop_from_last_token().is_err() {
                    tokens_clx.pop_token();
                }
                return;
            }
        };

        tokens_clx.replace_last_token(kind, content);
        line_clx.mark_dirty_at(tokens_clx.last().unwrap().clx().line_pos);
    }

    fn add_redirect_out_append_or_truncate(line_clx: &mut LineContext, tokens_clx: &mut TokensContext, ch: char) {
        if let Some(last) = tokens_clx.last() {
            if *last.kind() == TokenKind::RedirectOutTruncate {
                tokens_clx.replace_last_token(TokenKind::RedirectOutAppend, ">>".to_string());
                line_clx.mark_dirty_at(tokens_clx.last().unwrap().clx().line_pos);
                return;
            }
        }

        tokens_clx.push_token(TokenKind::RedirectOutTruncate, ch.to_string());
    }

    fn add_redirect_in_file(tokens_clx: &mut TokensContext, ch: char) {
        tokens_clx.push_token(TokenKind::RedirectInFile, ch.to_string());
    }

    fn add_background_or_logical_and(line_clx: &mut LineContext, tokens_clx: &mut TokensContext, ch: char) {
        if let Some(last) = tokens_clx.last() {
            if *last.kind() == TokenKind::Background {
                tokens_clx.replace_last_token(TokenKind::And, "&&".to_string());
                line_clx.mark_dirty_at(tokens_clx.last().unwrap().clx().line_pos);
                return;
            }
        }

        tokens_clx.push_token(TokenKind::Background, ch.to_string());
    }

    fn add_separator(tokens_clx: &mut TokensContext, ch: char) {
        tokens_clx.push_token(TokenKind::Separator, ch.to_string());
    }

    fn add_pipe_or_logical_or(line_clx: &mut LineContext, tokens_clx: &mut TokensContext, ch: char) {
        if let Some(last) = tokens_clx.last() {
            if *last.kind() == TokenKind::Pipe {
                tokens_clx.replace_last_token(TokenKind::Or, "||".to_string());
                line_clx.mark_dirty_at(tokens_clx.last().unwrap().clx().line_pos);
                return;
            }
        }

        tokens_clx.push_token(TokenKind::Pipe, ch.to_string());
    }

    fn add_ambiguous(tokens_clx: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens_clx.last() else {
            tokens_clx.push_token(TokenKind::Command, ch.to_string());
            return;
        };

        // If last token is dynamic => add to token
        if last_token.is_ambiguous() {
            tokens_clx
                .push_to_last_token(ch)
                .expect("Expected ambiguous Tokens to have dynamic content");
            return;
        }

        // Else => create new ambiguous token
        let next_kind = if last_token.clx().require_file {
            TokenKind::File
        } else if last_token.clx().cmd_pos_in_segment.is_some() {
            TokenKind::Argument
        } else {
            TokenKind::Command
        };
        tokens_clx.push_token(next_kind, ch.to_string());
    }

    fn add_blank(tokens_clx: &mut TokensContext, ch: char) {
        if let Some(last) = tokens_clx.last() {
            if *last.kind() == TokenKind::Blank {
                tokens_clx.push_to_last_token(ch);
                return;
            }
        }

        tokens_clx.push_token(TokenKind::Blank, ch.to_string());
    }

    fn add_quote(tokens_clx: &mut TokensContext, ch: char) {
        if let Some(last) = tokens_clx.last() {
            // Exit quote if matching quote char
            if last.is_in_quote_of(ch) {
                tokens_clx.push_token(TokenKind::QuoteEnd, ch.to_string());
                return;
            }
            // Continue current quote if inside with different char
            if last.clx().in_quote.is_some() {
                tokens_clx
                    .push_to_last_token(ch)
                    .expect("Expected Tokens in quote to have dynamic content");
                return;
            }
        }
        // Start a new quote otherwise
        tokens_clx.push_token(TokenKind::QuoteStart, ch.to_string());
    }
}
