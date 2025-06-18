use core::{cell::RefCell, char};

use alloc::{
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use logger::info;

use crate::{
    context::{context::Context, tokens_context::TokensContext},
    event::{
        event::Event,
        event_handler::{Error, EventHandler, Response},
    },
    modules::alias::Alias,
};

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
    pub(crate) quote: QuoteState,
    pub(crate) ambiguous: AmbiguousState,
    pub(crate) argument_type: Option<ArgumentType>,
    pub(crate) assigned_command_pos: Option<usize>,
    pub(crate) assigned_short_flag_pos: Option<usize>,
}

pub struct Lexer {
    alias: Rc<RefCell<Alias>>,
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

impl EventHandler for Lexer {
    fn on_prepare_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        clx.tokens.reset();
        Ok(Response::Ok)
    }

    fn on_submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.retokenize_with_alias(clx)
    }

    fn on_line_written(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let detokenize_res = match self.detokenize_to_dirty(clx) {
            Ok(res) => res,
            Err(err) => return Err(err),
        };
        let tokenize_res = match self.tokenize_from_dirty(clx) {
            Ok(res) => res,
            Err(err) => return Err(err),
        };

        if detokenize_res == Response::Skip && tokenize_res == Response::Skip {
            return Ok(Response::Skip);
        }

        clx.events.trigger(Event::TokensWritten);
        Ok(Response::Ok)
    }
}

impl Lexer {
    pub const fn new(alias: Rc<RefCell<Alias>>) -> Self {
        Self { alias }
    }

    fn detokenize_to_dirty(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let total_len = clx.tokens.total_len();

        if total_len <= clx.line.get_dirty_index() {
            return Ok(Response::Skip);
        }

        let n = total_len - clx.line.get_dirty_index();
        for _ in 0..n {
            self.pop(&mut clx.tokens);
        }

        Ok(Response::Ok)
    }

    fn tokenize_from_dirty(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if !clx.line.is_dirty() {
            return Ok(Response::Skip);
        }

        for ch in clx.line.get_dirty_part().chars() {
            self.push(&mut clx.tokens, ch);
        }

        info!("Lexer tokens: {:?}", clx.tokens);
        Ok(Response::Ok)
    }

    fn retokenize_with_alias(&mut self, clx: &mut Context) -> Result<Response, Error> {
        clx.tokens.reset();

        let new_line = clx
            .line
            .get()
            .split_whitespace()
            .map(|raw_token| match self.alias.borrow().get(raw_token) {
                Some(alias_value) => alias_value.to_string(),
                None => raw_token.to_string(),
            })
            .collect::<Vec<String>>()
            .join(" ");

        for ch in new_line.chars() {
            self.push(&mut clx.tokens, ch);
        }

        info!("Lexer tokens with alias: {:?}", clx.tokens);
        Ok(Response::Ok)
    }

    fn push(&mut self, tokens: &mut TokensContext, ch: char) {
        match ch {
            '\"' => self.handle_double_quote(tokens),
            '\'' => self.handle_single_quote(tokens),
            ' ' => self.handle_whitespace(tokens),
            ch => self.handle_other(tokens, ch),
        }
    }

    fn pop(&mut self, tokens: &mut TokensContext) {
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

    fn handle_other(&mut self, tokens: &mut TokensContext, ch: char) {
        if tokens.last().is_none() {
            tokens.push(self.choose_ambiguous_token(
                &TokenContext::new(QuoteState::Pending, AmbiguousState::Pending, None, None, None),
                ch,
                tokens.len(),
            ));
            return;
        }

        let len = tokens.len();
        let token = match tokens.last_mut().unwrap() {
            Token::Command(_clx, cmd) => return cmd.push(ch),
            Token::Argument(clx, arg) => return self.update_argument(clx, arg, ch),
            Token::QuoteStart(clx, _) => self.choose_ambiguous_token(clx, ch, len),
            Token::QuoteEnd(..) => panic!("Not supported"),
            Token::Whitespace(clx) => self.choose_ambiguous_token(clx, ch, len),
        };
        tokens.push(token);
    }

    fn update_argument(&mut self, clx: &mut TokenContext, arg: &mut String, ch: char) {
        arg.push(ch);

        if arg == "--" && clx.argument_type == Some(ArgumentType::Unknown) {
            clx.argument_type = Some(ArgumentType::LongFlag);
        } else if arg.starts_with('-') {
            clx.argument_type = Some(ArgumentType::ShortFlag)
        }
    }

    fn handle_whitespace(&mut self, tokens: &mut TokensContext) {
        if tokens.last().is_none() {
            return tokens.push(Token::Whitespace(TokenContext::new(
                QuoteState::Pending,
                AmbiguousState::Pending,
                None,
                None,
                None,
            )));
        }

        let last_token = tokens.last().unwrap();
        let len = tokens.len();
        if last_token.context().quote != QuoteState::Pending {
            let token = match tokens.last_mut().unwrap() {
                Token::QuoteStart(clx, _) => self.choose_ambiguous_token(clx, ' ', len),
                Token::Command(_clx, cmd) => return cmd.push(' '),
                Token::Argument(_clx, arg) => return arg.push(' '),
                _ => panic!("Invalid token state"),
            };
            return tokens.push(token);
        }

        let mut clx = last_token.context().clone();
        clx.argument_type = self.choose_argument_type(&clx, ' ');

        tokens.push(Token::Whitespace(clx));
    }

    fn choose_ambiguous_token(&mut self, clx: &TokenContext, ch: char, len: usize) -> Token {
        match clx.ambiguous {
            AmbiguousState::Pending => {
                let next_clx = TokenContext::new(clx.quote.clone(), AmbiguousState::Command, None, Some(len), None);
                Token::Command(next_clx, ch.to_string())
            }
            AmbiguousState::Command | AmbiguousState::Argument => {
                let argument_type = self.choose_argument_type(clx, ch);

                let next_clx = TokenContext::new(
                    clx.quote.clone(),
                    AmbiguousState::Argument,
                    argument_type.clone(),
                    clx.assigned_command_pos,
                    self.choose_argument_pos(argument_type, len),
                );
                Token::Argument(next_clx, ch.to_string())
            }
        }
    }

    fn choose_argument_type(&mut self, clx: &TokenContext, ch: char) -> Option<ArgumentType> {
        if clx.ambiguous != AmbiguousState::Command && clx.ambiguous != AmbiguousState::Argument {
            return None;
        }
        if clx.argument_type == Some(ArgumentType::ShortFlag) {
            return Some(ArgumentType::ShortFlagValue);
        }
        if clx.argument_type == Some(ArgumentType::ShortFlagValue) {
            return None;
        }
        if ch == '-' {
            return Some(ArgumentType::ShortFlag);
        }
        Some(ArgumentType::Unknown)
    }

    fn choose_argument_pos(&mut self, argument_type: Option<ArgumentType>, len: usize) -> Option<usize> {
        if argument_type == Some(ArgumentType::ShortFlag) {
            Some(len)
        } else {
            None
        }
    }

    fn handle_double_quote(&mut self, tokens: &mut TokensContext) {
        if tokens.last().is_none() {
            let clx = TokenContext::new(QuoteState::Double, AmbiguousState::Pending, None, None, None);
            tokens.push(Token::QuoteStart(clx, '\"'));
            return;
        }

        let len = tokens.len();
        let last_token = tokens.last_mut().unwrap();
        let last_clx = last_token.context();
        let argument_type = self.choose_argument_type(last_clx, '\"');
        let argument_pos = self.choose_argument_pos(argument_type.clone(), len);

        match last_clx.quote {
            QuoteState::Double => {
                // Exit double quote & Enable alias in quotes
                let clx = TokenContext::new(
                    QuoteState::Pending,
                    last_clx.ambiguous.clone(),
                    argument_type,
                    last_clx.assigned_command_pos,
                    argument_pos,
                );
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
                let clx = TokenContext::new(
                    QuoteState::Double,
                    last_clx.ambiguous.clone(),
                    argument_type,
                    last_clx.assigned_command_pos,
                    argument_pos,
                );
                tokens.push(Token::QuoteStart(clx, '\"'));
            }
        }
    }

    fn handle_single_quote(&mut self, tokens: &mut TokensContext) {
        if tokens.last().is_none() {
            let clx = TokenContext::new(QuoteState::Single, AmbiguousState::Pending, None, None, None);
            tokens.push(Token::QuoteStart(clx, '\''));
            return;
        }

        let len = tokens.len();
        let last_token = tokens.last_mut().unwrap();
        let last_clx = last_token.context();
        let argument_type = self.choose_argument_type(last_clx, '\'');
        let argument_pos = self.choose_argument_pos(argument_type.clone(), len);

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
                let clx = TokenContext::new(
                    QuoteState::Pending,
                    last_clx.ambiguous.clone(),
                    argument_type,
                    last_clx.assigned_command_pos,
                    argument_pos,
                );
                tokens.push(Token::QuoteEnd(clx, '\''));
            }
            QuoteState::Pending => {
                // Enter single quote & Disable alias in quotes
                let clx = TokenContext::new(
                    QuoteState::Single,
                    last_clx.ambiguous.clone(),
                    argument_type,
                    last_clx.assigned_command_pos,
                    argument_pos,
                );
                tokens.push(Token::QuoteStart(clx, '\''));
            }
        }
    }
}
