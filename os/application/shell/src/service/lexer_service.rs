use core::char;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use logger::{error, info};
use terminal::DecodedKey;

use crate::context::Context;

use super::service::{Service, ServiceError};

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Command(String),
    Argument(String),
    Whitespace,
    QuoteStart(char),
    QuoteEnd(char),
}

#[derive(Debug, PartialEq)]
pub enum AmbiguousState {
    Pending,
    Command,
    Argument,
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

pub struct LexerService {
    quote_state: QuoteState,
    ambiguous_state: Vec<AmbiguousState>,
    // alias: Rc<RefCell<Alias>>,
    // alias_state: AliasState,
}

impl Service for LexerService {
    fn run(&mut self, _event: DecodedKey, context: &mut Context) -> Result<(), ServiceError> {
        self.try_detokenize(context);
        return self.tokenize(context);
    }
}

impl LexerService {
    pub const fn new(/*alias: Rc<RefCell<Alias>>*/) -> Self {
        Self {
            quote_state: QuoteState::Pending,
            ambiguous_state: Vec::new(),
            // alias,
            // alias_state: AliasState::Pending,
        }
    }

    fn try_detokenize(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        let inner_len = context.inner_tokens_len();
        if inner_len <= context.dirty_offset {
            return Ok(());
        }

        let n = inner_len - context.dirty_offset;
        for _ in 0..n {
            self.pop(&mut context.tokens);
        }
        Ok(())
    }

    fn tokenize(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        for ch in context.line[context.dirty_offset..].iter() {
            self.push(&mut context.tokens, *ch);
        }

        info!(
            "Lexer [ tokens: {:?}, quote_state: {:?} ]",
            context.tokens, self.quote_state
        );
        Ok(())
    }

    fn push(&mut self, tokens: &mut Vec<Token>, ch: char) {
        match ch {
            '\"' => self.handle_double_quote(tokens),
            '\'' => self.handle_single_quote(tokens),
            ' ' => self.handle_whitespace(tokens),
            ch => self.handle_other(tokens, ch),
        }
    }

    fn pop(&mut self, tokens: &mut Vec<Token>) {
        if tokens.last().is_none() {
            return;
        }
        match tokens.last_mut().unwrap() {
            Token::Command(cmd) => {
                if cmd.pop().is_some() && !cmd.is_empty() {
                    return;
                }
                self.ambiguous_state.pop();
            }
            Token::Argument(arg) => {
                if arg.pop().is_some() && !arg.is_empty() {
                    return;
                }
                self.ambiguous_state.pop();
            }
            Token::QuoteEnd(ch) => match ch {
                '\'' => self.quote_state = QuoteState::Single,
                '\"' => self.quote_state = QuoteState::Double,
                _ => (),
            },
            Token::QuoteStart(_) => {
                self.quote_state = QuoteState::Pending;
            }
            _ => (),
        };
        tokens.pop();
    }

    fn handle_other(&mut self, tokens: &mut Vec<Token>, ch: char) {
        if tokens.last().is_none() {
            tokens.push(self.choose_ambiguous_token(ch));
            return;
        }

        let token = match tokens.last_mut().unwrap() {
            Token::Command(cmd) => return cmd.push(ch),
            Token::Argument(arg) => return arg.push(ch),
            Token::QuoteStart(_) => self.choose_ambiguous_token(ch),
            Token::QuoteEnd(_) => return error!("Not supported"),
            Token::Whitespace => self.choose_ambiguous_token(ch),
        };
        tokens.push(token);
    }

    fn handle_whitespace(&mut self, tokens: &mut Vec<Token>) {
        if tokens.last().is_none() {
            return tokens.push(Token::Whitespace);
        }

        if self.quote_state != QuoteState::Pending {
            let token = match tokens.last_mut().unwrap() {
                Token::QuoteStart(_) => self.choose_ambiguous_token(' '),
                Token::Command(cmd) => return cmd.push(' '),
                Token::Argument(arg) => return arg.push(' '),
                _ => return error!("Invalid token state"),
            };
            return tokens.push(token);
        }

        tokens.push(Token::Whitespace);
    }

    fn choose_ambiguous_token(&mut self, ch: char) -> Token {
        match self.ambiguous_state.last() {
            Some(AmbiguousState::Pending) | None => {
                self.ambiguous_state.push(AmbiguousState::Command);
                Token::Command(ch.to_string())
            }
            Some(AmbiguousState::Command) => {
                self.ambiguous_state.push(AmbiguousState::Argument);
                Token::Argument(ch.to_string())
            }
            Some(AmbiguousState::Argument) => {
                self.ambiguous_state.push(AmbiguousState::Argument);
                Token::Argument(ch.to_string())
            }
        }
    }

    fn handle_double_quote(&mut self, tokens: &mut Vec<Token>) {
        match self.quote_state {
            QuoteState::Double => {
                // Exit double quote & Enable alias in quotes
                self.quote_state = QuoteState::Pending;
                // self.alias_state = AliasState::Pending;
                tokens.push(Token::QuoteEnd('\"'));
            }
            QuoteState::Single => {
                // Pass through
                match tokens.last_mut().expect("Invalid token state") {
                    Token::Command(cmd) => cmd.push('\"'),
                    Token::Argument(cmd) => cmd.push('\"'),
                    _ => return error!("Invalid token state"),
                }
            }
            QuoteState::Pending => {
                // Enter double quote & Disable alias in quotes
                self.quote_state = QuoteState::Double;
                // self.alias_state = AliasState::Disabled;
                tokens.push(Token::QuoteStart('\"'));
            }
        }
    }

    fn handle_single_quote(&mut self, tokens: &mut Vec<Token>) {
        match self.quote_state {
            QuoteState::Double => {
                // Pass through
                match tokens.last_mut().expect("Invalid token state") {
                    Token::Command(cmd) => cmd.push('\''),
                    Token::Argument(cmd) => cmd.push('\''),
                    _ => return error!("Invalid token state"),
                }
            }
            QuoteState::Single => {
                // Exit single quote & Enable alias in quotes
                self.quote_state = QuoteState::Pending;
                // self.alias_state = AliasState::Pending;
                tokens.push(Token::QuoteEnd('\''));
            }
            QuoteState::Pending => {
                // Enter single quote & Disable alias in quotes
                self.quote_state = QuoteState::Single;
                // self.alias_state = AliasState::Disabled;
                tokens.push(Token::QuoteStart('\''));
            }
        }
    }

    // TODO Fix aliases
    // fn try_add_alias(&mut self) {
    //     if self.alias_state != AliasState::Pending {
    //         // Prevent iterating through aliases (otherwise: alias loop="echo loop" => echo echo echo echo ...)
    //         return;
    //     }

    //     let value = match { self.alias.borrow().get(&self.raw_token).map(String::from) } {
    //         Some(value) => value,
    //         None => return,
    //     };

    //     self.alias_state = AliasState::Writing;
    //     self.raw_token.clear();
    //     self.tokenize(&value);
    //     self.alias_state = AliasState::Writing;
    // }
}
