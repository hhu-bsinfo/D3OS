use core::{cell::RefCell, char};

use alloc::{
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use logger::{error, info};

use crate::{context::Context, sub_service::alias_sub_service::AliasSubService};

use super::service::{Error, Response, Service};

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

pub struct LexerService {
    alias: Rc<RefCell<AliasSubService>>,
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
}

impl TokenContext {
    pub const fn new(quote: QuoteState, ambiguous: AmbiguousState) -> Self {
        Self { quote, ambiguous }
    }
}

impl Service for LexerService {
    fn submit(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.retokenize_with_alias(context)
    }

    fn simple_key(&mut self, context: &mut Context, _key: char) -> Result<Response, Error> {
        self.detokenize_to_dirty(context);
        self.tokenize_from_dirty(context)
    }
}

impl LexerService {
    pub const fn new(alias: Rc<RefCell<AliasSubService>>) -> Self {
        Self { alias }
    }

    fn detokenize_to_dirty(&mut self, context: &mut Context) -> Result<Response, Error> {
        let inner_len = context.inner_tokens_len();
        if inner_len <= context.dirty_offset {
            return Ok(Response::Skip);
        }

        let n = inner_len - context.dirty_offset;
        for _ in 0..n {
            self.pop(&mut context.tokens);
        }
        Ok(Response::Ok)
    }

    fn tokenize_from_dirty(&mut self, context: &mut Context) -> Result<Response, Error> {
        for ch in context.line[context.dirty_offset..].iter() {
            self.push(&mut context.tokens, *ch);
        }

        info!("Lexer tokens: {:?}", context.tokens);
        Ok(Response::Ok)
    }

    fn retokenize_with_alias(&mut self, context: &mut Context) -> Result<Response, Error> {
        context.tokens.clear();
        let line: String = context.line.clone().into_iter().collect();

        let new_line = line
            .split_whitespace()
            .map(|raw_token| match self.alias.borrow().get(raw_token) {
                Some(alias_value) => alias_value.to_string(),
                None => raw_token.to_string(),
            })
            .collect::<Vec<String>>()
            .join(" ");

        for ch in new_line.chars() {
            self.push(&mut context.tokens, ch);
        }

        info!("Lexer tokens with alias: {:?}", context.tokens);

        Ok(Response::Ok)
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
            }
            QuoteState::Pending => {
                // Enter single quote & Disable alias in quotes
                let clx = TokenContext::new(QuoteState::Single, last_clx.ambiguous.clone());
                tokens.push(Token::QuoteStart(clx, '\''));
            }
        }
    }
}
