use alloc::string::{String, ToString};

#[derive(Debug, PartialEq, Clone)]
pub enum AmbiguousState {
    Pending,
    Command,
    Argument,
}

#[derive(Debug, PartialEq, Clone)]
pub enum QuoteState {
    Pending,
    Single,
    Double,
}

#[derive(Debug, PartialEq)]
pub enum AliasState {
    Pending,
    Writing,
    Disabled,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TokenContext {
    pub(crate) quote: QuoteState,
    pub(crate) ambiguous: AmbiguousState,
    pub(crate) argument_type: Option<ArgumentType>,
    pub(crate) assigned_command_pos: Option<usize>,
    pub(crate) assigned_short_flag_pos: Option<usize>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum TokenType {
    Command,
    Argument,
    Whitespace,
    QuoteStart,
    QuoteEnd,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ArgumentType {
    Unknown,
    Generic,
    ShortFlag,
    ShortFlagValue,
    LongFlag,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Command(TokenContext, String),
    Argument(TokenContext, String),
    Whitespace(TokenContext),
    QuoteStart(TokenContext, char),
    QuoteEnd(TokenContext, char),
}

impl Token {
    pub fn context(&self) -> &TokenContext {
        match self {
            Token::Command(ctx, _) => ctx,
            Token::Argument(ctx, _) => ctx,
            Token::Whitespace(ctx) => ctx,
            Token::QuoteStart(ctx, _) => ctx,
            Token::QuoteEnd(ctx, _) => ctx,
        }
    }

    pub fn token_type(&self) -> TokenType {
        match self {
            Token::Command(..) => TokenType::Command,
            Token::Argument(..) => TokenType::Argument,
            Token::Whitespace(..) => TokenType::Whitespace,
            Token::QuoteStart(..) => TokenType::QuoteStart,
            Token::QuoteEnd(..) => TokenType::QuoteEnd,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Token::Command(_, string) => string.clone(),
            Token::Argument(_, string) => string.clone(),
            Token::Whitespace(..) => " ".to_string(),
            Token::QuoteStart(_, ch) => ch.to_string(),
            Token::QuoteEnd(_, ch) => ch.to_string(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Token::Command(_, string) => string.len(),
            Token::Argument(_, string) => string.len(),
            Token::Whitespace(..) => 1,
            Token::QuoteStart(..) => 1,
            Token::QuoteEnd(..) => 1,
        }
    }

    pub fn is_ambiguous(&self) -> bool {
        match self {
            Token::Command(..) | Token::Argument(..) => true,
            _ => false,
        }
    }
}

impl TokenContext {
    pub const fn new(
        quote: QuoteState,
        ambiguous: AmbiguousState,
        argument_type: Option<ArgumentType>,
        assigned_command_pos: Option<usize>,
        assigned_short_flag_pos: Option<usize>,
    ) -> Self {
        Self {
            quote,
            ambiguous,
            argument_type,
            assigned_command_pos,
            assigned_short_flag_pos,
        }
    }
}
