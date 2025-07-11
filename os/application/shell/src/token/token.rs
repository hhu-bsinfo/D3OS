use alloc::string::{String, ToString};

use crate::{
    event::event_handler::Error,
    token::{
        and_token::AndTokenContextFactory, argument_token::ArgumentTokenContextFactory,
        background_token::BackgroundTokenContextFactory, blank_token::BlankTokenContextFactory,
        command_token::CommandTokenContextFactory, file_token::FileTokenContextFactory,
        or_token::OrTokenContextFactory, pipe_token::PipeTokenContextFactory,
        quote_end_token::QuoteEndTokenContextFactory, quote_start_token::QuoteStartTokenContextFactory,
        redirect_in_append_token::RedirectInAppendTokenContextFactory,
        redirect_in_truncate_token::RedirectInTruncateTokenContextFactory,
        redirect_out_append_token::RedirectOutAppendTokenContextFactory,
        redirect_out_truncate_token::RedirectOutTruncateTokenContextFactory,
        separator_token::SeparatorTokenContextFactory,
    },
};

#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
    Command,
    Argument,
    Blank,
    QuoteStart,
    QuoteEnd,
    Pipe,
    Separator,
    Background,
    And,
    Or,
    RedirectInTruncate,
    RedirectInAppend,
    RedirectOutTruncate,
    RedirectOutAppend,
    File,
}

#[derive(Debug, PartialEq, Clone)]
pub enum TokenStatus {
    Valid,
    Incomplete,
    Error(&'static Error),
}

impl TokenStatus {
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    pub fn is_incomplete(&self) -> bool {
        *self == Self::Incomplete
    }

    pub fn is_valid(&self) -> bool {
        *self == Self::Valid
    }
}

#[allow(unused_variables)]
pub trait TokenContextFactory {
    fn create_first(content: &str) -> TokenContext;
    fn create_after(prev_token: &Token, content: &str) -> TokenContext;
}

/// TODO docs: Difference to State: No changes after creation
#[derive(Debug, Clone)]
pub struct TokenContext {
    // Position in tokens
    pub pos: usize,
    pub line_pos: usize,
    // Position of Command in tokens, for the current segment
    pub cmd_pos: Option<usize>,
    // Char of quote (if token is quote)
    pub in_quote: Option<char>,
    pub error: Option<&'static Error>,
    pub require_cmd: bool,
    pub require_file: bool,
    pub has_background: bool,
}

impl TokenContext {
    fn create_after(prev_token: &Token, kind: &TokenKind, content: &str) -> Self {
        match kind {
            TokenKind::Command => CommandTokenContextFactory::create_after(prev_token, content),
            TokenKind::Argument => ArgumentTokenContextFactory::create_after(prev_token, content),
            TokenKind::Blank => BlankTokenContextFactory::create_after(prev_token, content),
            TokenKind::QuoteStart => QuoteStartTokenContextFactory::create_after(prev_token, content),
            TokenKind::QuoteEnd => QuoteEndTokenContextFactory::create_after(prev_token, content),
            TokenKind::Pipe => PipeTokenContextFactory::create_after(prev_token, content),
            TokenKind::Separator => SeparatorTokenContextFactory::create_after(prev_token, content),
            TokenKind::Background => BackgroundTokenContextFactory::create_after(prev_token, content),
            TokenKind::And => AndTokenContextFactory::create_after(prev_token, content),
            TokenKind::Or => OrTokenContextFactory::create_after(prev_token, content),
            TokenKind::RedirectInAppend => RedirectInAppendTokenContextFactory::create_after(prev_token, content),
            TokenKind::RedirectInTruncate => RedirectInTruncateTokenContextFactory::create_after(prev_token, content),
            TokenKind::RedirectOutAppend => RedirectOutAppendTokenContextFactory::create_after(prev_token, content),
            TokenKind::RedirectOutTruncate => RedirectOutTruncateTokenContextFactory::create_after(prev_token, content),
            TokenKind::File => FileTokenContextFactory::create_after(prev_token, content),
        }
    }

    fn create_first(kind: &TokenKind, content: &str) -> Self {
        match *kind {
            TokenKind::Command => CommandTokenContextFactory::create_first(content),
            TokenKind::Argument => ArgumentTokenContextFactory::create_first(content),
            TokenKind::Blank => BlankTokenContextFactory::create_first(content),
            TokenKind::QuoteStart => QuoteStartTokenContextFactory::create_first(content),
            TokenKind::QuoteEnd => QuoteEndTokenContextFactory::create_first(content),
            TokenKind::Pipe => PipeTokenContextFactory::create_first(content),
            TokenKind::Separator => SeparatorTokenContextFactory::create_first(content),
            TokenKind::Background => BackgroundTokenContextFactory::create_first(content),
            TokenKind::And => AndTokenContextFactory::create_first(content),
            TokenKind::Or => OrTokenContextFactory::create_first(content),
            TokenKind::RedirectInAppend => RedirectInAppendTokenContextFactory::create_first(content),
            TokenKind::RedirectInTruncate => RedirectInTruncateTokenContextFactory::create_first(content),
            TokenKind::RedirectOutAppend => RedirectOutAppendTokenContextFactory::create_first(content),
            TokenKind::RedirectOutTruncate => RedirectOutTruncateTokenContextFactory::create_first(content),
            TokenKind::File => FileTokenContextFactory::create_first(content),
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
    pub fn new_first(kind: TokenKind, content: String) -> Self {
        let clx = TokenContext::create_first(&kind, &content);
        Self { kind, content, clx }
    }

    pub fn new_after(prev_token: &Token, kind: TokenKind, content: String) -> Self {
        let clx = TokenContext::create_after(prev_token, &kind, &content);
        Self { kind, content, clx }
    }

    fn check_status(&self) -> TokenStatus {
        if self.clx.require_cmd {
            return TokenStatus::Incomplete;
        }
        if self.clx.in_quote.is_some() {
            return TokenStatus::Incomplete;
        }
        if self.clx.require_file {
            return TokenStatus::Incomplete;
        }

        TokenStatus::Valid
    }

    pub fn kind(&self) -> &TokenKind {
        &self.kind
    }

    pub fn clx(&self) -> &TokenContext {
        &self.clx
    }

    pub fn status(&self) -> TokenStatus {
        match self.clx.error {
            Some(error) => TokenStatus::Error(error),
            None => self.check_status(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.content
    }

    pub fn as_str_at_line_index(&self, index: usize) -> &str {
        if index < self.clx.line_pos {
            return &self.content;
        }

        &self.content[index - self.clx.line_pos..]
    }

    pub fn to_string(&self) -> String {
        self.content.to_string()
    }

    pub fn len(&self) -> usize {
        self.content.len()
    }

    pub fn push(&mut self, ch: char) {
        self.content.push(ch);
    }

    pub fn pop(&mut self) -> Result<char, ()> {
        if self.content.len() <= 1 {
            return Err(());
        }
        let ch = self.content.pop().unwrap();
        Ok(ch)
    }

    pub fn is_ambiguous(&self) -> bool {
        self.kind == TokenKind::Command || self.kind == TokenKind::Argument || self.kind == TokenKind::File
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
