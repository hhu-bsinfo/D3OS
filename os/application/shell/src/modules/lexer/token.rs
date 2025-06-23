use alloc::string::{String, ToString};

use crate::event::event_handler::Error;

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
pub enum AmbiguousKind {
    None,
    Command,
    Argument,
}

#[derive(Debug, PartialEq, Clone)]
pub enum QuoteKind {
    None,
    Single,
    Double,
}

impl QuoteKind {
    pub fn from_char(ch: &char) -> Self {
        match *ch {
            '\"' => QuoteKind::Double,
            '\'' => QuoteKind::Single,
            e => panic!("Received unknown quote literal: {}", e),
        }
    }
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
    fn new(
        pos: usize,
        cmd_pos: Option<usize>,
        short_flag_pos: Option<usize>,
        in_quote: Option<char>,
        arg_kind: ArgumentKind,
        status: TokenStatus,
    ) -> Self {
        Self {
            pos,
            cmd_pos,
            short_flag_pos,
            in_quote,
            arg_kind,
            status,
        }
    }

    fn create_after(kind: &TokenKind, ch: char, prev_clx: &TokenContext) -> Self {
        match *kind {
            TokenKind::Command => Self::create_command_after(prev_clx),
            TokenKind::Argument => Self::create_argument_after(prev_clx, ch),
            TokenKind::Blank => Self::create_blank_after(prev_clx),
            TokenKind::QuoteStart => Self::create_quote_start_after(prev_clx, ch),
            TokenKind::QuoteEnd => Self::create_quote_end_after(prev_clx),
        }
    }

    fn create_command_after(prev_clx: &TokenContext) -> Self {
        Self {
            pos: prev_clx.pos + 1,
            cmd_pos: Some(prev_clx.pos + 1),
            short_flag_pos: None,
            in_quote: prev_clx.in_quote,
            arg_kind: ArgumentKind::None,
            status: prev_clx.status.clone(),
        }
    }

    fn create_argument_after(prev_clx: &TokenContext, ch: char) -> Self {
        let arg_kind: ArgumentKind;
        let short_flag_pos: Option<usize>;

        if prev_clx.arg_kind == ArgumentKind::ShortFlag {
            arg_kind = ArgumentKind::ShortFlagValue;
            short_flag_pos = prev_clx.short_flag_pos;
        } else if ch == '-' {
            arg_kind = ArgumentKind::ShortOrLongFlag;
            short_flag_pos = None;
        } else {
            arg_kind = ArgumentKind::Generic;
            short_flag_pos = None;
        };

        Self {
            pos: prev_clx.pos + 1,
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos,
            in_quote: prev_clx.in_quote,
            arg_kind,
            status: prev_clx.status.clone(),
        }
    }

    fn create_blank_after(prev_clx: &TokenContext) -> Self {
        Self {
            pos: prev_clx.pos + 1,
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos: prev_clx.short_flag_pos,
            in_quote: prev_clx.in_quote,
            arg_kind: prev_clx.arg_kind.clone(),
            status: prev_clx.status.clone(),
        }
    }

    fn create_quote_start_after(prev_clx: &TokenContext, ch: char) -> Self {
        Self {
            pos: prev_clx.pos + 1,
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos: prev_clx.short_flag_pos,
            in_quote: Some(ch),
            arg_kind: prev_clx.arg_kind.clone(),
            status: prev_clx.status.clone(),
        }
    }

    fn create_quote_end_after(prev_clx: &TokenContext) -> Self {
        Self {
            pos: prev_clx.pos + 1,
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos: prev_clx.short_flag_pos,
            in_quote: None,
            arg_kind: prev_clx.arg_kind.clone(),
            status: prev_clx.status.clone(),
        }
    }

    fn increment_pos(&mut self) {
        self.pos += 1;
    }

    fn create_first(kind: &TokenKind, ch: char) -> Self {
        match *kind {
            TokenKind::Command => Self::create_first_command(),
            TokenKind::QuoteStart => Self::create_first_quote(ch),
            TokenKind::Blank => Self::create_first_blank(),
            TokenKind::Argument => panic!("The first token can not be a argument"),
            TokenKind::QuoteEnd => panic!("The first token can not be end of quote"),
        }
    }

    fn create_first_command() -> Self {
        Self {
            pos: 0,
            cmd_pos: Some(0),
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            status: TokenStatus::Valid,
        }
    }

    fn create_first_quote(ch: char) -> Self {
        Self {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: Some(ch),
            arg_kind: ArgumentKind::None,
            status: TokenStatus::Incomplete,
        }
    }

    fn create_first_blank() -> Self {
        Self {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            status: TokenStatus::Valid,
        }
    }

    fn revalidate(&mut self, kind: &TokenKind, string: &str) {
        match *kind {
            TokenKind::Command => {}
            TokenKind::Argument => self.revalidate_argument(string),
            TokenKind::Blank => {}
            TokenKind::QuoteStart => {}
            TokenKind::QuoteEnd => {}
        }
    }

    fn revalidate_argument(&mut self, string: &str) {
        if self.arg_kind == ArgumentKind::ShortFlagValue {
            return;
        }

        if string == "-" {
            self.arg_kind = ArgumentKind::ShortOrLongFlag;
            self.short_flag_pos = None;
            return;
        }
        if string.starts_with("--") {
            self.arg_kind = ArgumentKind::LongFlag;
            self.short_flag_pos = None;
            return;
        }
        if string.starts_with("-") {
            self.arg_kind = ArgumentKind::ShortFlag;
            self.short_flag_pos = Some(self.pos);
            return;
        }
        self.arg_kind = ArgumentKind::Generic;
        self.short_flag_pos = None;
    }

    fn revalidate_blank(&mut self) {}
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

    pub fn new_after(kind: TokenKind, ch: char, prev_clx: &TokenContext) -> Self {
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
