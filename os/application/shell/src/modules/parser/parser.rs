use core::{cell::RefCell, char};

use alloc::{
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use logger::info;

use crate::{
    context::{
        context::Context,
        executable_context::{Io, JobBuilder, JobResult},
        tokens_context::TokensContext,
    },
    event::{
        event::Event,
        event_handler::{Error, EventHandler, Response},
    },
    modules::parser::token::{Token, TokenKind, TokenStatus},
    sub_modules::alias::Alias,
};

pub struct Parser {
    // Sub module for processing aliases
    alias: Rc<RefCell<Alias>>,
}

impl EventHandler for Parser {
    fn on_prepare_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        clx.executable.reset();
        clx.tokens.reset();
        Ok(Response::Ok)
    }

    fn on_submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.retokenize_with_alias(clx);
        self.parse(clx)
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

impl Parser {
    pub const fn new(alias: Rc<RefCell<Alias>>) -> Self {
        Self { alias }
    }

    fn parse(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let mut job_builder = JobBuilder::new();
        job_builder.id(clx.executable.len());

        for token in clx.tokens.get() {
            match token.status() {
                TokenStatus::Error(error) => return Err((*error).clone()),
                _ => (),
            }

            match token.kind() {
                TokenKind::Command => {
                    let Ok(job) = job_builder.build() else {
                        job_builder.command(token.to_string());
                        continue;
                    };
                    clx.executable.add_job(job);
                    job_builder = JobBuilder::new();
                    job_builder.id(clx.executable.len());
                }

                TokenKind::Argument => {
                    job_builder.add_argument(token.to_string());
                }

                TokenKind::Background => {
                    job_builder.run_in_background(true);
                }

                TokenKind::Separator => {
                    let job = job_builder.build();
                    if job.is_ok() {
                        clx.executable.add_job(job.unwrap());
                    }
                    job_builder = JobBuilder::new();
                    job_builder.id(clx.executable.len());
                }

                TokenKind::And => {
                    let Some(last_job) = clx.executable.last_job() else {
                        return Err(Error::new("And condition requires a preceding job", None));
                    };
                    job_builder.requires_job(last_job.id, JobResult::Success);
                }

                TokenKind::Or => {
                    let Some(last_job) = clx.executable.last_job() else {
                        return Err(Error::new("Or condition requires a preceding job", None));
                    };
                    job_builder.requires_job(last_job.id, JobResult::Error);
                }

                TokenKind::Pipe => {
                    let Some(last_job) = clx.executable.last_job_mut() else {
                        return Err(Error::new("Pipe requires a preceding job", None));
                    };

                    last_job.output = Io::Job(job_builder.peek_id().expect("Next job id should be set by now"));
                    job_builder.use_input(Io::Job(last_job.id));
                }

                TokenKind::QuoteStart | TokenKind::QuoteEnd | TokenKind::Blank => (),
            }
        }

        match job_builder.build() {
            Ok(job) => clx.executable.add_job(job),
            Err(_) => (),
        };

        info!("{:?}", &clx.executable);
        Ok(Response::Ok)
    }

    fn detokenize_to_dirty(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let total_len = clx.tokens.total_len();

        if total_len <= clx.line.get_dirty_index() {
            return Ok(Response::Skip);
        }

        let n = total_len - clx.line.get_dirty_index();
        for _ in 0..n {
            self.remove(&mut clx.tokens);
        }

        Ok(Response::Ok)
    }

    fn tokenize_from_dirty(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if !clx.line.is_dirty() {
            return Ok(Response::Skip);
        }

        for ch in clx.line.get_dirty_part().chars() {
            self.add(&mut clx.tokens, ch);
        }

        for token in clx.tokens.get() {
            info!("{:?}", token);
        }
        Ok(Response::Ok)
    }

    // TODO FIX: echo " hhu " => " Heinrich Heine Universitaet ", but should be " hhu "
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
            self.add(&mut clx.tokens, ch);
        }

        info!("Lexer tokens with alias: {:?}", clx.tokens);
        Ok(Response::Ok)
    }

    fn add(&mut self, tokens: &mut TokensContext, ch: char) {
        if tokens
            .last()
            .is_some_and(|token| token.clx().in_quote.is_some_and(|quote| quote != ch))
        {
            self.add_ambiguous(tokens, ch);
            return;
        }

        match ch {
            // Job control
            ';' => self.add_separator(tokens, ch),
            '&' => self.add_background_or_logical_and(tokens, ch),
            '|' => self.add_pipe_or_logical_or(tokens, ch),
            // Redirection
            '>' => { /* TODO redirect_out_truncate || redirect_out_append */ }
            '<' => { /* TODO redirect_in_truncate || redirect_in_append */ }
            // Quotes
            '\"' | '\'' => self.add_quote(tokens, ch),
            // Other
            ' ' | '\t' => self.add_blank(tokens, ch),
            ch => self.add_ambiguous(tokens, ch),
        }
    }

    fn remove(&mut self, tokens: &mut TokensContext) {
        let Some(last_token) = tokens.last_mut() else {
            return;
        };

        match *last_token.kind() {
            TokenKind::And => {
                tokens.pop();
                let replace_token = match tokens.last() {
                    Some(token) => Token::new_after(token.clx(), TokenKind::Background, '&'),
                    None => Token::new_first(TokenKind::Background, '&'),
                };
                tokens.push(replace_token);
            }
            TokenKind::Or => {
                tokens.pop();
                let replace_token = match tokens.last() {
                    Some(token) => Token::new_after(token.clx(), TokenKind::Pipe, '|'),
                    None => Token::new_first(TokenKind::Pipe, '|'),
                };
                tokens.push(replace_token);
            }
            _ => {
                match last_token.pop() {
                    Ok(_) => return,
                    Err(_) => tokens.pop(),
                };
            }
        }
    }

    fn add_background_or_logical_and(&mut self, tokens: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Background, ch);
            tokens.push(first_token);
            return;
        };

        // If last token is background => remove it and add logical and token
        if *last_token.kind() == TokenKind::Background {
            tokens.pop();
            let mut next_token = match tokens.last() {
                Some(token) => Token::new_after(token.clx(), TokenKind::And, ch),
                None => Token::new_first(TokenKind::And, ch),
            };
            next_token.push(ch);
            tokens.push(next_token);
            return;
        }

        // Else add next background token
        let next_token = Token::new_after(last_token.clx(), TokenKind::Background, ch);
        tokens.push(next_token);
    }

    fn add_separator(&mut self, tokens: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Separator, ch);
            tokens.push(first_token);
            return;
        };

        // Else add next separator token
        let next_token = Token::new_after(last_token.clx(), TokenKind::Separator, ch);
        tokens.push(next_token);
    }

    fn add_pipe_or_logical_or(&mut self, tokens: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Pipe, ch);
            tokens.push(first_token);
            return;
        };

        // If last token is pipe => remove it and add logical or token
        if *last_token.kind() == TokenKind::Pipe {
            tokens.pop();
            let mut next_token = match tokens.last() {
                Some(token) => Token::new_after(token.clx(), TokenKind::Or, ch),
                None => Token::new_first(TokenKind::Or, ch),
            };
            next_token.push(ch);
            tokens.push(next_token);
            return;
        }

        // Else add next pipe token
        let next_token = Token::new_after(last_token.clx(), TokenKind::Pipe, ch);
        tokens.push(next_token);
    }

    fn add_ambiguous(&mut self, tokens: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Command, ch);
            tokens.push(first_token);
            return;
        };

        // If last token is ambiguous => add to token
        if last_token.is_ambiguous() {
            last_token.push(ch);
            return;
        }

        // Else => create new ambiguous token
        let next_kind = match last_token.has_segment_cmd() {
            true => TokenKind::Argument,
            false => TokenKind::Command,
        };
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(prev_clx, next_kind, ch);
        tokens.push(next_token);
    }

    fn add_blank(&mut self, tokens: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Blank, ch);
            tokens.push(first_token);
            return;
        };

        // Else => Append blank token
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(prev_clx, TokenKind::Blank, ch);
        tokens.push(next_token);
    }

    fn add_quote(&mut self, tokens: &mut TokensContext, ch: char) {
        // If no token => create first token
        let Some(last_token) = tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::QuoteStart, ch);
            tokens.push(first_token);
            return;
        };

        // If in quote and char matches quote char => exit quote
        if last_token.is_in_quote_of(ch) {
            let prev_clx = last_token.clx();
            let next_token = Token::new_after(prev_clx, TokenKind::QuoteEnd, ch);
            tokens.push(next_token);
            return;
        }
        // If in quote with different char => add to in quote token
        else if last_token.is_in_quote() {
            last_token.push(ch);
            return;
        }

        // Else => Enter quote
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(prev_clx, TokenKind::QuoteStart, ch);
        tokens.push(next_token);
    }
}
