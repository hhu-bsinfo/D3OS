use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use logger::info;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Command(String),
    Argument(String),
}

pub struct Lexer {
    tokens: Vec<Token>,
}

impl Lexer {
    pub const fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    pub fn tokenize(&mut self, input: String) -> Result<(), ()> {
        self.tokens.clear(); // TODO#? Retokenize each time for now (maybe stream chars later)
        for item in input.split_whitespace() {
            self.push(item);
        }

        info!("{:?}", self.tokens);
        Ok(())
    }

    pub fn flush(&mut self) -> Vec<Token> {
        let tokens = self.tokens.clone();
        self.tokens.clear();
        tokens
    }

    fn push(&mut self, item: &str) {
        match item {
            // TODO unambiguous here
            _ => self.add_ambiguous(item),
        }
    }

    fn add_ambiguous(&mut self, item: &str) {
        let token = match self.tokens.last() {
            Some(Token::Command(_)) => Token::Argument(item.to_string()),
            Some(Token::Argument(_)) => Token::Argument(item.to_string()),
            _ => Token::Command(item.to_string()),
        };
        self.tokens.push(token);
    }
}
