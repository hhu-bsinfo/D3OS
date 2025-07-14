use alloc::string::{String, ToString};

use crate::event::event_handler::Error;

#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
    // Ambiguous
    Command,
    Argument,
    File,
    // Quote
    QuoteStart,
    QuoteEnd,
    // Redirection
    RedirectInTruncate,
    RedirectInAppend,
    RedirectOutTruncate,
    RedirectOutAppend,
    // Logical Operator
    And,
    Or,
    // Other
    Blank,
    Separator,
    Pipe,
    Background,
}

#[derive(Debug, PartialEq, Clone)]
pub enum TokenStatus {
    Valid,
    Incomplete(Error),
    Error(Error),
}

impl TokenStatus {
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    pub fn is_incomplete(&self) -> bool {
        matches!(self, Self::Incomplete(_))
    }

    pub fn is_valid(&self) -> bool {
        *self == Self::Valid
    }
}

#[derive(Debug, Clone)]
pub struct TokenContext {
    pub pos: usize,
    pub line_pos: usize,
    pub cmd_pos_in_segment: Option<usize>,
    pub require_segment: bool,
    pub require_file: bool,
    pub in_quote: Option<char>,
    pub is_end_of_line: bool,
}

#[derive(Debug, Clone)]
pub struct Token {
    kind: TokenKind,
    content: String,
    clx: TokenContext,
    status: TokenStatus,
}

impl Token {
    pub fn new(kind: TokenKind, content: String, clx: TokenContext, status: TokenStatus) -> Self {
        Self {
            kind,
            content,
            clx,
            status,
        }
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

    pub fn as_str_at_line_pos(&self, pos: usize) -> &str {
        if pos < self.clx.line_pos {
            return &self.content;
        }

        &self.content[pos - self.clx.line_pos..]
    }

    pub fn to_string(&self) -> String {
        self.content.to_string()
    }

    pub fn len(&self) -> usize {
        self.content.len()
    }

    pub fn push(&mut self, ch: char) -> Result<(), ()> {
        if !self.is_content_dynamic() {
            return Err(());
        }
        self.content.push(ch);
        Ok(())
    }

    pub fn pop(&mut self) -> Result<char, ()> {
        if !self.is_content_dynamic() || self.content.len() <= 1 {
            return Err(());
        }
        let ch = self.content.pop().unwrap();
        Ok(ch)
    }

    pub fn is_in_quote_of(&self, ch: char) -> bool {
        self.clx.in_quote.is_some_and(|quote| quote == ch)
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self.kind, TokenKind::Command | TokenKind::Argument | TokenKind::File)
    }

    pub fn is_content_dynamic(&self) -> bool {
        self.is_ambiguous() || self.kind == TokenKind::Blank
    }
}
