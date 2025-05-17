use core::fmt;

use alloc::string::String;

#[derive(Debug, PartialEq)]
pub enum Token {
    Whitespace,
    Command(String),
    Argument(String),
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Whitespace => write!(f, ""),
            Token::Command(cmd) => write!(f, "{}", cmd),
            Token::Argument(arg) => write!(f, "{}", arg),
        }
    }
}
