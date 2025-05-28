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
    Command(TokenContext, String),
    Argument(TokenContext, String),
    Whitespace(TokenContext),
    QuoteStart(TokenContext, char),
    QuoteEnd(TokenContext, char),
}

#[derive(Debug, PartialEq, Clone)]
pub enum AmbiguousState {
    Pending,
    Command,
    Argument,
}

#[derive(Debug, PartialEq, Clone)]
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

#[derive(Debug, PartialEq, Clone)]
pub struct TokenContext {
    quote: QuoteState,
    ambiguous: AmbiguousState,
}

pub struct LexerService {}

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
}

impl TokenContext {
    pub const fn new(quote: QuoteState, ambiguous: AmbiguousState) -> Self {
        Self { quote, ambiguous }
    }
}

impl Service for LexerService {
    fn run(&mut self, event: DecodedKey, context: &mut Context) -> Result<(), ServiceError> {
        match event {
            DecodedKey::Unicode('\n') => self.on_enter(),
            _ => self.on_other_key(context),
        }
    }
}

impl LexerService {
    pub const fn new(/*alias: Rc<RefCell<Alias>>*/) -> Self {
        Self {}
    }

    fn on_enter(&mut self) -> Result<(), ServiceError> {
        Ok(())
    }

    fn on_other_key(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        self.try_detokenize(context);
        return self.tokenize(context);
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

        info!("Lexer tokens: {:?}", context.tokens);
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
            Token::Command(_clx, cmd) => {
                if cmd.pop().is_some() && !cmd.is_empty() {
                    return;
                }
            }
            Token::Argument(_clx, arg) => {
                if arg.pop().is_some() && !arg.is_empty() {
                    return;
                }
            }
            _ => (),
        };
        tokens.pop();
    }

    fn handle_other(&mut self, tokens: &mut Vec<Token>, ch: char) {
        if tokens.last().is_none() {
            tokens.push(self.choose_ambiguous_token(
                &TokenContext::new(QuoteState::Pending, AmbiguousState::Pending),
                ch,
            ));
            return;
        }

        let token = match tokens.last_mut().unwrap() {
            Token::Command(_clx, cmd) => return cmd.push(ch),
            Token::Argument(_clx, arg) => return arg.push(ch),
            Token::QuoteStart(clx, _) => self.choose_ambiguous_token(clx, ch),
            Token::QuoteEnd(..) => return error!("Not supported"),
            Token::Whitespace(clx) => self.choose_ambiguous_token(clx, ch),
        };
        tokens.push(token);
    }

    fn handle_whitespace(&mut self, tokens: &mut Vec<Token>) {
        if tokens.last().is_none() {
            return tokens.push(Token::Whitespace(TokenContext::new(
                QuoteState::Pending,
                AmbiguousState::Pending,
            )));
        }

        let last_token = tokens.last().unwrap();
        if last_token.context().quote != QuoteState::Pending {
            let token = match tokens.last_mut().unwrap() {
                Token::QuoteStart(clx, _) => self.choose_ambiguous_token(clx, ' '),
                Token::Command(_clx, cmd) => return cmd.push(' '),
                Token::Argument(_clx, arg) => return arg.push(' '),
                _ => return error!("Invalid token state"),
            };
            return tokens.push(token);
        }

        tokens.push(Token::Whitespace(last_token.context().clone()));
    }

    fn choose_ambiguous_token(&mut self, clx: &TokenContext, ch: char) -> Token {
        match clx.ambiguous {
            AmbiguousState::Pending => {
                let next_clx = TokenContext::new(clx.quote.clone(), AmbiguousState::Command);
                Token::Command(next_clx, ch.to_string())
            }
            AmbiguousState::Command => {
                let next_clx = TokenContext::new(clx.quote.clone(), AmbiguousState::Argument);
                Token::Argument(next_clx, ch.to_string())
            }
            AmbiguousState::Argument => {
                let next_clx = TokenContext::new(clx.quote.clone(), AmbiguousState::Argument);
                Token::Argument(next_clx, ch.to_string())
            }
        }
    }

    fn handle_double_quote(&mut self, tokens: &mut Vec<Token>) {
        if tokens.last().is_none() {
            let clx = TokenContext::new(QuoteState::Double, AmbiguousState::Pending);
            tokens.push(Token::QuoteStart(clx, '\"'));
            return;
        }

        let last_token = tokens.last_mut().unwrap();
        let last_clx = last_token.context();
        match last_clx.quote {
            QuoteState::Double => {
                // Exit double quote & Enable alias in quotes
                let clx = TokenContext::new(QuoteState::Pending, last_clx.ambiguous.clone());
                tokens.push(Token::QuoteEnd(clx, '\"'));
                // self.alias_state = AliasState::Pending;
            }
            QuoteState::Single => {
                // Pass through
                match last_token {
                    Token::Command(_clx, cmd) => cmd.push('\"'),
                    Token::Argument(_clx, arg) => arg.push('\"'),
                    _ => panic!("Invalid token state"),
                }
            }
            QuoteState::Pending => {
                // Enter double quote & Disable alias in quotes
                let clx = TokenContext::new(QuoteState::Double, last_clx.ambiguous.clone());
                tokens.push(Token::QuoteStart(clx, '\"'));
                // self.alias_state = AliasState::Disabled;
            }
        }
    }

    fn handle_single_quote(&mut self, tokens: &mut Vec<Token>) {
        if tokens.last().is_none() {
            let clx = TokenContext::new(QuoteState::Single, AmbiguousState::Pending);
            tokens.push(Token::QuoteStart(clx, '\''));
            return;
        }

        let last_token = tokens.last_mut().unwrap();
        let last_clx = last_token.context();
        match last_clx.quote {
            QuoteState::Double => {
                // Pass through
                match last_token {
                    Token::Command(_clx, cmd) => cmd.push('\''),
                    Token::Argument(_clx, arg) => arg.push('\''),
                    _ => panic!("Invalid token state"),
                }
            }
            QuoteState::Single => {
                // Exit single quote & Enable alias in quotes
                let clx = TokenContext::new(QuoteState::Pending, last_clx.ambiguous.clone());
                tokens.push(Token::QuoteEnd(clx, '\''));
                // self.alias_state = AliasState::Pending;
            }
            QuoteState::Pending => {
                // Enter single quote & Disable alias in quotes
                let clx = TokenContext::new(QuoteState::Single, last_clx.ambiguous.clone());
                tokens.push(Token::QuoteStart(clx, '\''));
                // self.alias_state = AliasState::Disabled;
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
