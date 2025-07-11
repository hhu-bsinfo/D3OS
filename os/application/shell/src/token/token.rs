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
    fn create_first(kind: &TokenKind, ch: char) -> TokenContext;
    fn create_after(prev_clx: &TokenContext, prev_content: &str, kind: &TokenKind, ch: char) -> TokenContext;
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
    fn create_after(prev_clx: &TokenContext, prev_content: &str, kind: &TokenKind, ch: char) -> Self {
        match *kind {
            TokenKind::Command => CommandTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
            TokenKind::Argument => ArgumentTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
            TokenKind::Blank => BlankTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
            TokenKind::QuoteStart => QuoteStartTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
            TokenKind::QuoteEnd => QuoteEndTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
            TokenKind::Pipe => PipeTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
            TokenKind::Separator => SeparatorTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
            TokenKind::Background => BackgroundTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
            TokenKind::And => AndTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
            TokenKind::Or => OrTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
            TokenKind::RedirectInAppend => {
                RedirectInAppendTokenContextFactory::create_after(prev_clx, prev_content, kind, ch)
            }
            TokenKind::RedirectInTruncate => {
                RedirectInTruncateTokenContextFactory::create_after(prev_clx, prev_content, kind, ch)
            }
            TokenKind::RedirectOutAppend => {
                RedirectOutAppendTokenContextFactory::create_after(prev_clx, prev_content, kind, ch)
            }
            TokenKind::RedirectOutTruncate => {
                RedirectOutTruncateTokenContextFactory::create_after(prev_clx, prev_content, kind, ch)
            }
            TokenKind::File => FileTokenContextFactory::create_after(prev_clx, prev_content, kind, ch),
        }
    }

    fn create_first(kind: &TokenKind, ch: char) -> Self {
        match *kind {
            TokenKind::Command => CommandTokenContextFactory::create_first(kind, ch),
            TokenKind::Argument => ArgumentTokenContextFactory::create_first(kind, ch),
            TokenKind::Blank => BlankTokenContextFactory::create_first(kind, ch),
            TokenKind::QuoteStart => QuoteStartTokenContextFactory::create_first(kind, ch),
            TokenKind::QuoteEnd => QuoteEndTokenContextFactory::create_first(kind, ch),
            TokenKind::Pipe => PipeTokenContextFactory::create_first(kind, ch),
            TokenKind::Separator => SeparatorTokenContextFactory::create_first(kind, ch),
            TokenKind::Background => BackgroundTokenContextFactory::create_first(kind, ch),
            TokenKind::And => AndTokenContextFactory::create_first(kind, ch),
            TokenKind::Or => OrTokenContextFactory::create_first(kind, ch),
            TokenKind::RedirectInAppend => RedirectInAppendTokenContextFactory::create_first(kind, ch),
            TokenKind::RedirectInTruncate => RedirectInTruncateTokenContextFactory::create_first(kind, ch),
            TokenKind::RedirectOutAppend => RedirectOutAppendTokenContextFactory::create_first(kind, ch),
            TokenKind::RedirectOutTruncate => RedirectOutTruncateTokenContextFactory::create_first(kind, ch),
            TokenKind::File => FileTokenContextFactory::create_first(kind, ch),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    kind: TokenKind,
    content: String,
    clx: TokenContext,
    status: TokenStatus,
}

impl Token {
    pub fn new_first(kind: TokenKind, ch: char) -> Self {
        let clx = TokenContext::create_first(&kind, ch);
        let content = ch.to_string();
        let status = match clx.error {
            Some(error) => TokenStatus::Error(error),
            None => Self::check_status(&clx),
        };

        Self {
            kind,
            content,
            clx,
            status,
        }
    }

    pub fn new_after(prev_clx: &TokenContext, prev_content: &str, kind: TokenKind, ch: char) -> Self {
        let clx = TokenContext::create_after(prev_clx, prev_content, &kind, ch);
        let content = ch.to_string();
        let status = match clx.error {
            Some(error) => TokenStatus::Error(error),
            None => Self::check_status(&clx),
        };

        Self {
            kind,
            content,
            clx,
            status,
        }
    }

    fn check_status(clx: &TokenContext) -> TokenStatus {
        if clx.require_cmd {
            return TokenStatus::Incomplete;
        }
        if clx.in_quote.is_some() {
            return TokenStatus::Incomplete;
        }
        if clx.require_file {
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

    pub fn status(&self) -> &TokenStatus {
        &self.status
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
