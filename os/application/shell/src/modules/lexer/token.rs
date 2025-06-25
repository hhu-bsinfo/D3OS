use alloc::string::{String, ToString};

use crate::{
    event::event_handler::Error,
    modules::lexer::{
        argument_token::ArgumentTokenContextFactory, blank_token::BlankTokenContextFactory,
        command_token::CommandTokenContextFactory, quote_end_token::QuoteEndTokenContextFactory,
        quote_start_token::QuoteStartTokenContextFactory,
    },
};

#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
    Command,
    Argument,
    Blank,
    QuoteStart,
    QuoteEnd,
}

#[derive(Debug, PartialEq, Clone)]
pub enum TokenStatus {
    Valid,
    Incomplete,
    Error(Error),
}

#[derive(Debug, PartialEq, Clone)]
pub enum ArgumentKind {
    None,
    ShortOrLongFlag,
    Generic,
    ShortFlag,
    ShortFlagValue,
    LongFlag,
}

#[allow(unused_variables)]
pub trait TokenContextFactory {
    fn create_first(kind: &TokenKind, ch: char) -> TokenContext;
    fn create_after(prev_clx: &TokenContext, kind: &TokenKind, ch: char) -> TokenContext;
    fn revalidate(clx: &mut TokenContext, kind: &TokenKind, string: &str) {}
}

/// TODO docs: Difference to State: No changes after creation
#[derive(Debug, Clone)]
pub struct TokenContext {
    // Position in tokens
    pub pos: usize,
    // Position of Command in tokens, for the current segment
    pub cmd_pos: Option<usize>,
    // Position of assigned ShortFlag in tokens (if ShortFlagValue)
    pub short_flag_pos: Option<usize>,
    // Char of quote (if token is quote)
    pub in_quote: Option<char>,
    // Kind of argument (if argument)
    pub arg_kind: ArgumentKind,
    pub status: TokenStatus,
}

impl TokenContext {
    fn create_after(kind: &TokenKind, ch: char, prev_clx: &TokenContext) -> Self {
        match *kind {
            TokenKind::Command => CommandTokenContextFactory::create_after(prev_clx, kind, ch),
            TokenKind::Argument => ArgumentTokenContextFactory::create_after(prev_clx, kind, ch),
            TokenKind::Blank => BlankTokenContextFactory::create_after(prev_clx, kind, ch),
            TokenKind::QuoteStart => QuoteStartTokenContextFactory::create_after(prev_clx, kind, ch),
            TokenKind::QuoteEnd => QuoteEndTokenContextFactory::create_after(prev_clx, kind, ch),
        }
    }

    fn create_first(kind: &TokenKind, ch: char) -> Self {
        match *kind {
            TokenKind::Command => CommandTokenContextFactory::create_first(kind, ch),
            TokenKind::Argument => ArgumentTokenContextFactory::create_first(kind, ch),
            TokenKind::Blank => BlankTokenContextFactory::create_first(kind, ch),
            TokenKind::QuoteStart => QuoteStartTokenContextFactory::create_first(kind, ch),
            TokenKind::QuoteEnd => QuoteEndTokenContextFactory::create_first(kind, ch),
        }
    }

    fn revalidate(&mut self, kind: &TokenKind, string: &str) {
        match *kind {
            TokenKind::Command => CommandTokenContextFactory::revalidate(self, kind, string),
            TokenKind::Argument => ArgumentTokenContextFactory::revalidate(self, kind, string),
            TokenKind::Blank => BlankTokenContextFactory::revalidate(self, kind, string),
            TokenKind::QuoteStart => QuoteStartTokenContextFactory::revalidate(self, kind, string),
            TokenKind::QuoteEnd => QuoteEndTokenContextFactory::revalidate(self, kind, string),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    kind: TokenKind,
    content: String,
    clx: TokenContext,
}

impl Token {
    pub fn new_first(kind: TokenKind, ch: char) -> Self {
        let clx = TokenContext::create_first(&kind, ch);
        let content = ch.to_string();
        Self { kind, content, clx }
    }

    pub fn new_after(prev_clx: &TokenContext, kind: TokenKind, ch: char) -> Self {
        let clx = TokenContext::create_after(&kind, ch, prev_clx);
        let content = ch.to_string();
        Self { kind, content, clx }
    }

    pub fn kind(&self) -> &TokenKind {
        &self.kind
    }

    pub fn clx(&self) -> &TokenContext {
        &self.clx
    }

    pub fn as_str(&self) -> &str {
        &self.content
    }

    pub fn len(&self) -> usize {
        self.content.len()
    }

    pub fn push(&mut self, ch: char) {
        self.content.push(ch);
        self.revalidate();
    }

    pub fn pop(&mut self) -> Result<char, ()> {
        if self.content.len() <= 1 {
            return Err(());
        }
        let ch = self.content.pop().unwrap();
        self.revalidate();
        Ok(ch)
    }

    fn revalidate(&mut self) {
        self.clx.revalidate(&self.kind, &self.content);
    }

    pub fn is_ambiguous(&self) -> bool {
        self.kind == TokenKind::Command || self.kind == TokenKind::Argument
    }

    pub fn has_segment_cmd(&self) -> bool {
        self.clx.cmd_pos.is_some()
    }

    pub fn is_in_quote(&self) -> bool {
        self.clx.in_quote.is_some()
    }

    pub fn is_in_quote_of(&self, ch: char) -> bool {
        self.clx.in_quote.is_some_and(|quote| quote == ch)
    }

    pub fn expect_command(&self) -> bool {
        self.clx.cmd_pos.is_none()
    }
}
