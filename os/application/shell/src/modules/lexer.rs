use core::{cell::RefCell, char};

use alloc::{
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use logger::{info, warn};

use crate::{
    context::{alias_context::AliasContext, line_context::LineContext, tokens_context::TokensContext},
    event::{
        event::Event,
        event_bus::EventBus,
        event_handler::{Error, EventHandler, Response},
    },
    token::token::{Token, TokenKind},
};

pub struct Lexer {
    line_provider: Rc<RefCell<LineContext>>,
    tokens_provider: Rc<RefCell<TokensContext>>,
    alias_provider: Rc<RefCell<AliasContext>>,
}

impl EventHandler for Lexer {
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

impl Lexer {
    pub const fn new(
        line_provider: Rc<RefCell<LineContext>>,
        tokens_provider: Rc<RefCell<TokensContext>>,
        alias_provider: Rc<RefCell<AliasContext>>,
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

    // TODO FIX: echo " hhu " => " Heinrich Heine Universitaet ", but should be " hhu "
    fn retokenize_with_alias(&mut self) -> Result<Response, Error> {
        let mut tokens_clx = self.tokens_provider.borrow_mut();
        let mut line_clx = self.line_provider.borrow_mut();

        tokens_clx.reset();

        let new_line = line_clx
            .get()
            .split_whitespace()
            .map(|raw_token| match self.alias_provider.borrow().get(raw_token) {
                Some(alias_value) => alias_value.to_string(),
                None => raw_token.to_string(),
            })
            .collect::<Vec<String>>()
            .join(" ");

        for ch in new_line.chars() {
            Self::add(&mut line_clx, &mut tokens_clx, ch);
        }

        info!("Lexer tokens with alias: {:#?}", tokens_clx);
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
            '<' => Self::add_redirect_in_append_or_truncate(line_clx, tokens_clx, ch),
            // Quotes
            '\"' | '\'' => Self::add_quote(tokens_clx, ch),
            // Other
            ' ' | '\t' => Self::add_blank(tokens_clx, ch),
            ch => Self::add_ambiguous(tokens_clx, ch),
        }
    }

    fn remove(line_clx: &mut LineContext, tokens_clx: &mut TokensContext) {
        let Some(last_token) = tokens_clx.last_mut() else {
            return;
        };

        match *last_token.kind() {
            TokenKind::And => {
                warn!("Before pop and");
                let rm = tokens_clx.pop();
                warn!("Removed and: {:?}", rm);
                let replace_token = match tokens_clx.last() {
                    Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::Background, '&'),
                    None => Token::new_first(TokenKind::Background, '&'),
                };
                line_clx.mark_dirty_at(replace_token.clx().line_pos);
                tokens_clx.push(replace_token);
            }
            TokenKind::Or => {
                tokens_clx.pop();
                let replace_token = match tokens_clx.last() {
                    Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::Pipe, '|'),
                    None => Token::new_first(TokenKind::Pipe, '|'),
                };
                line_clx.mark_dirty_at(replace_token.clx().line_pos);
                tokens_clx.push(replace_token);
            }
            TokenKind::RedirectInAppend => {
                tokens_clx.pop();
                let replace_token = match tokens_clx.last() {
                    Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::RedirectInTruncate, '<'),
                    None => Token::new_first(TokenKind::RedirectInTruncate, '<'),
                };
                line_clx.mark_dirty_at(replace_token.clx().line_pos);
                tokens_clx.push(replace_token);
            }
            TokenKind::RedirectOutAppend => {
                tokens_clx.pop();
                let replace_token = match tokens_clx.last() {
                    Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::RedirectOutTruncate, '>'),
                    None => Token::new_first(TokenKind::RedirectOutTruncate, '>'),
                };
                line_clx.mark_dirty_at(replace_token.clx().line_pos);
                tokens_clx.push(replace_token);
            }
            _ => {
                match last_token.pop() {
                    Ok(_) => return,
                    Err(_) => tokens_clx.pop(),
                };
            }
        }
    }

    fn add_redirect_out_append_or_truncate(line_clx: &mut LineContext, tokens_clx: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens_clx.last_mut() else {
            let first_token = Token::new_first(TokenKind::RedirectOutTruncate, ch);
            tokens_clx.push(first_token);
            return;
        };

        // If last token is truncate => remove it and add append
        if *last_token.kind() == TokenKind::RedirectOutTruncate {
            tokens_clx.pop();
            let mut next_token = match tokens_clx.last() {
                Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::RedirectOutAppend, ch),
                None => Token::new_first(TokenKind::RedirectOutAppend, ch),
            };
            next_token.push(ch);
            line_clx.mark_dirty_at(next_token.clx().line_pos);
            tokens_clx.push(next_token);
            return;
        }

        // Else add next background token
        let next_token = Token::new_after(
            last_token.clx(),
            last_token.as_str(),
            TokenKind::RedirectOutTruncate,
            ch,
        );
        tokens_clx.push(next_token);
    }

    fn add_redirect_in_append_or_truncate(line_clx: &mut LineContext, tokens_clx: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens_clx.last_mut() else {
            let first_token = Token::new_first(TokenKind::RedirectInTruncate, ch);
            tokens_clx.push(first_token);
            return;
        };

        // If last token is truncate => remove it and add append
        if *last_token.kind() == TokenKind::RedirectInTruncate {
            tokens_clx.pop();
            let mut next_token = match tokens_clx.last() {
                Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::RedirectInAppend, ch),
                None => Token::new_first(TokenKind::RedirectInAppend, ch),
            };
            next_token.push(ch);
            line_clx.mark_dirty_at(next_token.clx().line_pos);
            tokens_clx.push(next_token);
            return;
        }

        // Else add next background token
        let next_token = Token::new_after(last_token.clx(), last_token.as_str(), TokenKind::RedirectInTruncate, ch);
        tokens_clx.push(next_token);
    }

    fn add_background_or_logical_and(line_clx: &mut LineContext, tokens_clx: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens_clx.last_mut() else {
            let first_token = Token::new_first(TokenKind::Background, ch);
            tokens_clx.push(first_token);
            return;
        };

        // If last token is background => remove it and add logical and token
        if *last_token.kind() == TokenKind::Background {
            tokens_clx.pop();
            let mut next_token = match tokens_clx.last() {
                Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::And, ch),
                None => Token::new_first(TokenKind::And, ch),
            };
            next_token.push(ch);
            line_clx.mark_dirty_at(next_token.clx().line_pos);
            tokens_clx.push(next_token);
            return;
        }

        // Else add next background token
        let next_token = Token::new_after(last_token.clx(), last_token.as_str(), TokenKind::Background, ch);
        tokens_clx.push(next_token);
    }

    fn add_separator(tokens_clx: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens_clx.last_mut() else {
            let first_token = Token::new_first(TokenKind::Separator, ch);
            tokens_clx.push(first_token);
            return;
        };

        // Else add next separator token
        let next_token = Token::new_after(last_token.clx(), last_token.as_str(), TokenKind::Separator, ch);
        tokens_clx.push(next_token);
    }

    fn add_pipe_or_logical_or(line_clx: &mut LineContext, tokens_clx: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens_clx.last_mut() else {
            let first_token = Token::new_first(TokenKind::Pipe, ch);
            tokens_clx.push(first_token);
            return;
        };

        // If last token is pipe => remove it and add logical or token
        if *last_token.kind() == TokenKind::Pipe {
            tokens_clx.pop();
            let mut next_token = match tokens_clx.last() {
                Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::Or, ch),
                None => Token::new_first(TokenKind::Or, ch),
            };
            next_token.push(ch);
            line_clx.mark_dirty_at(next_token.clx().line_pos);
            tokens_clx.push(next_token);
            return;
        }

        // Else add next pipe token
        let next_token = Token::new_after(last_token.clx(), last_token.as_str(), TokenKind::Pipe, ch);
        tokens_clx.push(next_token);
    }

    fn add_ambiguous(tokens_clx: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens_clx.last_mut() else {
            let first_token = Token::new_first(TokenKind::Command, ch);
            tokens_clx.push(first_token);
            return;
        };

        // If last token is ambiguous => add to token
        if last_token.is_ambiguous() {
            last_token.push(ch);
            return;
        }

        // Else => create new ambiguous token
        let next_kind = if last_token.clx().require_file {
            TokenKind::File
        } else if last_token.has_segment_cmd() {
            TokenKind::Argument
        } else {
            TokenKind::Command
        };
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(prev_clx, last_token.as_str(), next_kind, ch);
        tokens_clx.push(next_token);
    }

    fn add_blank(tokens_clx: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens_clx.last_mut() else {
            let first_token = Token::new_first(TokenKind::Blank, ch);
            tokens_clx.push(first_token);
            return;
        };

        // Else => Append blank token
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(prev_clx, last_token.as_str(), TokenKind::Blank, ch);
        tokens_clx.push(next_token);
    }

    fn add_quote(tokens_clx: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens_clx.last_mut() else {
            let first_token = Token::new_first(TokenKind::QuoteStart, ch);
            tokens_clx.push(first_token);
            return;
        };

        // If in quote and char matches quote char => exit quote
        if last_token.is_in_quote_of(ch) {
            let prev_clx = last_token.clx();
            let next_token = Token::new_after(prev_clx, last_token.as_str(), TokenKind::QuoteEnd, ch);
            tokens_clx.push(next_token);
            return;
        }
        // If in quote with different char => add to in quote token
        else if last_token.is_in_quote() {
            last_token.push(ch);
            return;
        }

        // Else => Enter quote
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(prev_clx, last_token.as_str(), TokenKind::QuoteStart, ch);
        tokens_clx.push(next_token);
    }
}
