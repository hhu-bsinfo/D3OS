use core::cell::RefCell;

use alloc::{rc::Rc, string::String, vec::Vec};
use logger::info;

use crate::sub_module::alias::Alias;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Command(String),
    Argument(String),
}

#[derive(Debug, PartialEq)]
enum QuoteState {
    Pending,
    Single,
    Double,
}

#[derive(Debug, PartialEq)]
enum AliasState {
    Pending,
    Writing,
    Disabled,
}

#[derive(Debug)]
pub struct Lexer {
    quote_state: QuoteState,
    tokens: Vec<Token>,
    raw_token: String,
    alias: Rc<RefCell<Alias>>,
    alias_state: AliasState,
}

impl Lexer {
    pub const fn new(alias: Rc<RefCell<Alias>>) -> Self {
        Self {
            quote_state: QuoteState::Pending,
            tokens: Vec::new(),
            raw_token: String::new(),
            alias,
            alias_state: AliasState::Pending,
        }
    }

    pub fn tokenize(&mut self, input: &str) {
        for ch in input.chars() {
            match ch {
                '\"' => self.handle_double_quote(),
                '\'' => self.handle_single_quote(),
                ' ' => self.handle_whitespace(),
                ch => self.handle_other(ch),
            }
        }

        info!(
            "Lexer [ tokens: {:?}, raw_token: {:?}, quote_state: {:?} ]",
            self.tokens, self.raw_token, self.quote_state
        );
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

    pub fn reset(&mut self) {
        self.tokens.clear();
        self.raw_token.clear();
        self.quote_state = QuoteState::Pending;
        self.alias_state = AliasState::Pending;
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.quote_state != QuoteState::Pending {
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
        if self.quote_state != QuoteState::Pending {
            // Don't split tokens inside quotes
            return self.raw_token.push(' ');
        }

        self.add_token();

        if self.quote_state == QuoteState::Pending {
            // No longer in quote with new token => can reenable alias
            self.alias_state = AliasState::Pending
        }
    }

    fn add_token(&mut self) {
        if self.raw_token.is_empty() {
            return;
        }

        self.try_add_alias();

        let token = self.choose_token();
        self.tokens.push(token);
        self.raw_token.clear();
    }

    fn try_add_alias(&mut self) {
        if self.alias_state != AliasState::Pending {
            // Prevent iterating through aliases (otherwise: alias loop="echo loop" => echo echo echo echo ...)
            return;
        }

        let value = match { self.alias.borrow().get(&self.raw_token).map(String::from) } {
            Some(value) => value,
            None => return,
        };

        self.alias_state = AliasState::Writing;
        self.raw_token.clear();
        self.tokenize(&value);
        self.alias_state = AliasState::Writing;
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
                self.quote_state = QuoteState::Pending;
            }
            QuoteState::Single => {
                // Pass through
                self.raw_token.push('\"');
            }
            QuoteState::Pending => {
                // Enter double quote
                self.quote_state = QuoteState::Double;
                // Disable alias in quotes (reenable on whitespace)
                self.alias_state = AliasState::Disabled;
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
                self.quote_state = QuoteState::Pending;
            }
            QuoteState::Pending => {
                // Enter single quote
                self.quote_state = QuoteState::Single;
                // Disable alias in quotes (reenable on whitespace)
                self.alias_state = AliasState::Disabled;
            }
        }
    }
}
