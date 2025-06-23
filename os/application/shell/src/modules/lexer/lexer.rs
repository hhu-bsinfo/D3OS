use core::{cell::RefCell, char};

use alloc::{
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use logger::info;

use crate::{
    context::{context::Context, tokens_context::TokensContext},
    event::{
        event::Event,
        event_handler::{Error, EventHandler, Response},
    },
    modules::lexer::token::{Token, TokenKind},
    sub_modules::alias::Alias,
};

pub struct Lexer {
    // Sub module for processing aliases
    alias: Rc<RefCell<Alias>>,
}

impl EventHandler for Lexer {
    fn on_prepare_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        clx.tokens.reset();
        Ok(Response::Ok)
    }

    fn on_submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.retokenize_with_alias(clx)
    }

    fn on_line_written(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let detokenize_res = match self.detokenize_to_dirty(clx) {
            Ok(res) => res,
            Err(err) => return Err(err),
        };
        let tokenize_res = match self.tokenize_from_dirty(clx) {
            Ok(res) => res,
            Err(err) => return Err(err),
        };

        if detokenize_res == Response::Skip && tokenize_res == Response::Skip {
            return Ok(Response::Skip);
        }

        clx.events.trigger(Event::TokensWritten);
        Ok(Response::Ok)
    }
}

impl Lexer {
    pub const fn new(alias: Rc<RefCell<Alias>>) -> Self {
        Self { alias }
    }

    fn detokenize_to_dirty(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let total_len = clx.tokens.total_len();

        if total_len <= clx.line.get_dirty_index() {
            return Ok(Response::Skip);
        }

        let n = total_len - clx.line.get_dirty_index();
        for _ in 0..n {
            self.remove(&mut clx.tokens);
        }

        Ok(Response::Ok)
    }

    fn tokenize_from_dirty(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if !clx.line.is_dirty() {
            return Ok(Response::Skip);
        }

        for ch in clx.line.get_dirty_part().chars() {
            self.add(&mut clx.tokens, ch);
        }

        for token in clx.tokens.get() {
            info!("{:?}", token);
        }
        Ok(Response::Ok)
    }

    fn retokenize_with_alias(&mut self, clx: &mut Context) -> Result<Response, Error> {
        clx.tokens.reset();

        let new_line = clx
            .line
            .get()
            .split_whitespace()
            .map(|raw_token| match self.alias.borrow().get(raw_token) {
                Some(alias_value) => alias_value.to_string(),
                None => raw_token.to_string(),
            })
            .collect::<Vec<String>>()
            .join(" ");

        for ch in new_line.chars() {
            self.add(&mut clx.tokens, ch);
        }

        info!("Lexer tokens with alias: {:?}", clx.tokens);
        Ok(Response::Ok)
    }

    fn add(&mut self, tokens: &mut TokensContext, ch: char) {
        match ch {
            // Job control
            ';' => { /* TODO separator */ }
            '&' => { /* TODO background || and */ }
            '|' => { /* TODO pipe || or */ }
            // Redirection
            '>' => { /* TODO redirect_out_truncate || redirect_out_append */ }
            '<' => { /* TODO redirect_in_truncate || redirect_in_append */ }
            // Quotes
            '\"' | '\'' => self.add_quote(tokens, ch),
            // Other
            ' ' | '\t' => self.add_blank(tokens, ch),
            ch => self.add_ambiguous(tokens, ch),
        }
    }

    fn remove(&mut self, tokens: &mut TokensContext) {
        let Some(last_token) = tokens.last_mut() else {
            return;
        };

        match last_token.pop() {
            Ok(_) => return,
            Err(_) => tokens.pop(),
        };
    }

    fn add_ambiguous(&mut self, tokens: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Command, ch);
            tokens.push(first_token);
            return;
        };

        // If last token is ambiguous => add to token
        if last_token.is_ambiguous() {
            last_token.push(ch);
            return;
        }

        // Else => create new ambiguous token
        let next_kind = match last_token.has_segment_cmd() {
            true => TokenKind::Argument,
            false => TokenKind::Command,
        };
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(next_kind, ch, prev_clx);
        tokens.push(next_token);
    }

    fn add_blank(&mut self, tokens: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Blank, ch);
            tokens.push(first_token);
            return;
        };

        // If in quote => add to in quote token
        if last_token.is_in_quote() {
            last_token.push(ch);
            return;
        }

        // Else => Append blank token
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(TokenKind::Blank, ch, prev_clx);
        tokens.push(next_token);
    }

    fn add_quote(&mut self, tokens: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::QuoteStart, ch);
            tokens.push(first_token);
            return;
        };

        // If in quote and char matches quote char => exit quote
        if last_token.is_in_quote_of(ch) {
            let prev_clx = last_token.clx();
            let next_token = Token::new_after(TokenKind::QuoteEnd, ch, prev_clx);
            tokens.push(next_token);
            return;
        }
        // If in quote with different char => add to in quote token
        else if last_token.is_in_quote() {
            last_token.push(ch);
            return;
        }

        // Else => Enter quote
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(TokenKind::QuoteStart, ch, prev_clx);
        tokens.push(next_token);
    }
}
