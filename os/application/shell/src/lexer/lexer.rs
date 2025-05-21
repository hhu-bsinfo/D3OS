use alloc::{string::String, vec::Vec};
use logger::info;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Command(String),
    Argument(String),
}

#[derive(Debug, PartialEq)]
enum QuoteState {
    None,
    Single,
    Double,
}

#[derive(Debug)]
pub struct Lexer {
    quote_state: QuoteState,
    tokens: Vec<Token>,
    raw_token: String,
}

impl Lexer {
    pub const fn new() -> Self {
        Self {
            quote_state: QuoteState::None,
            tokens: Vec::new(),
            raw_token: String::new(),
        }
    }

    pub fn tokenize(&mut self, input: &str) -> Result<(), ()> {
        self.reset();
        for ch in input.chars() {
            match ch {
                '\"' => self.handle_double_quote(),
                '\'' => self.handle_single_quote(),
                ' ' => self.handle_whitespace(),
                ch => self.handle_other(ch),
            }
        }

        info!("{:?}", self);
        Ok(())
    }

    pub fn flush(&mut self) -> Result<Vec<Token>, &'static str> {
        self.add_token();

        match self.validate() {
            Ok(_) => {}
            Err(msg) => return Err(msg),
        }

        let tokens = self.tokens.clone();
        self.reset();
        Ok(tokens)
    }

    fn reset(&mut self) {
        self.tokens.clear();
        self.raw_token.clear();
        self.quote_state = QuoteState::None
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.quote_state != QuoteState::None {
            // TODO Add general error object (Message, Reason, Hint)
            return Err(
                "Invalid input. Reason unclosed quote.\nIf you intended to write a literal, try wrapping the expression into quotes.\nExample: \"That's better\"",
            );
        }
        Ok(())
    }

    fn handle_other(&mut self, ch: char) {
        self.raw_token.push(ch);
    }

    fn handle_whitespace(&mut self) {
        if self.quote_state != QuoteState::None {
            // Don't split tokens inside quotes
            return self.raw_token.push(' ');
        }

        self.add_token();
    }

    fn add_token(&mut self) {
        if self.raw_token.is_empty() {
            return;
        }

        let token = self.choose_token();
        self.tokens.push(token);
        self.raw_token.clear();
    }

    fn choose_token(&mut self) -> Token {
        match self.raw_token {
            // TODO unambiguous here
            _ => self.choose_ambiguous(),
        }
    }

    fn choose_ambiguous(&mut self) -> Token {
        match self.tokens.last() {
            Some(Token::Command(_)) => Token::Argument(self.raw_token.clone()),
            Some(Token::Argument(_)) => Token::Argument(self.raw_token.clone()),
            _ => Token::Command(self.raw_token.clone()),
        }
    }

    fn handle_double_quote(&mut self) {
        match self.quote_state {
            QuoteState::Double => {
                // Exit double quote
                self.quote_state = QuoteState::None;
            }
            QuoteState::Single => {
                // Pass through
                self.raw_token.push('\"');
            }
            QuoteState::None => {
                // Enter double quote
                self.quote_state = QuoteState::Double
            }
        }
    }

    fn handle_single_quote(&mut self) {
        match self.quote_state {
            QuoteState::Double => {
                // Pass through
                self.raw_token.push('\'');
            }
            QuoteState::Single => {
                // Exit single quote
                self.quote_state = QuoteState::None;
            }
            QuoteState::None => {
                // Enter single quote
                self.quote_state = QuoteState::Single
            }
        }
    }
}
