use core::{cell::RefCell, char};

use alloc::{
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use logger::{info, warn};

use crate::{
    context::{
        context::Context,
        executable_context::{Io, JobBuilder, JobResult},
    },
    event::{
        event::Event,
        event_handler::{Error, EventHandler, Response},
    },
    modules::parser::token::{Token, TokenKind, TokenStatus},
    sub_modules::alias::Alias,
};

#[derive(Debug)]
enum IoType {
    None,
    InAppend,
    InTruncate,
    OutAppend,
    OutTruncate,
}

pub struct Parser {
    // Sub module for processing aliases
    alias: Rc<RefCell<Alias>>,
    current_io_type: IoType,
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
        self.tokenize_from_dirty(clx)
    }

    fn on_history_restored(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.tokenize_from_dirty(clx)
    }
}

impl Parser {
    pub const fn new(alias: Rc<RefCell<Alias>>) -> Self {
        Self {
            alias,
            current_io_type: IoType::None,
        }
    }

    fn tokenize_from_dirty(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let dirty_index = clx.line.get_dirty_index();
        let detokenize_res = match self.detokenize_to(dirty_index, clx) {
            Ok(res) => res,
            Err(err) => return Err(err),
        };
        let tokenize_res = match self.tokenize_from(dirty_index, clx) {
            Ok(res) => res,
            Err(err) => return Err(err),
        };

        if detokenize_res == Response::Skip && tokenize_res == Response::Skip {
            return Ok(Response::Skip);
        }

        clx.events.trigger(Event::TokensWritten);
        Ok(Response::Ok)
    }

    fn parse(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let mut job_builder = JobBuilder::new();
        job_builder.id(clx.executable.len());
        self.current_io_type = IoType::None;

        for token in clx.tokens.get() {
            match token.status() {
                TokenStatus::Error(error) => return Err((*error).clone()),
                _ => (),
            }

            if !token.has_segment_cmd() {
                let Ok(job) = job_builder.build() else {
                    continue;
                };
                clx.executable.add_job(job);
                job_builder = JobBuilder::new();
                job_builder.id(clx.executable.len());
            }

            match token.kind() {
                TokenKind::Command => {
                    job_builder.command(token.to_string());
                }

                TokenKind::Argument => {
                    job_builder.add_argument(token.to_string());
                }

                TokenKind::Background => {
                    for job in &mut clx.executable.jobs {
                        job.background_execution = true
                    }
                    job_builder.run_in_background(true);
                }

                TokenKind::And => {
                    let Some(last_job) = clx.executable.last_job() else {
                        return Err(Error::new("And condition requires a preceding job".to_string(), None));
                    };
                    job_builder.requires_job(last_job.id, JobResult::Success);
                }

                TokenKind::Or => {
                    let Some(last_job) = clx.executable.last_job() else {
                        return Err(Error::new("Or condition requires a preceding job".to_string(), None));
                    };
                    job_builder.requires_job(last_job.id, JobResult::Error);
                }

                TokenKind::Pipe => {
                    let Some(last_job) = clx.executable.last_job_mut() else {
                        return Err(Error::new("Pipe requires a preceding job".to_string(), None));
                    };

                    last_job.output = Io::Job(job_builder.peek_id().expect("Next job id should be set by now"));
                    job_builder.use_input(Io::Job(last_job.id));
                }

                TokenKind::File => {
                    warn!("{:?}", self.current_io_type);
                    match self.current_io_type {
                        IoType::InAppend => job_builder.use_input(Io::FileAppend(token.to_string())),
                        IoType::InTruncate => job_builder.use_input(Io::FileTruncate(token.to_string())),
                        IoType::OutAppend => job_builder.use_output(Io::FileAppend(token.to_string())),
                        IoType::OutTruncate => job_builder.use_output(Io::FileTruncate(token.to_string())),
                        IoType::None => {
                            return Err(Error::new(
                                "Received file without redirection instruction".to_string(),
                                None,
                            ));
                        }
                    };
                    self.current_io_type = IoType::None;
                }

                TokenKind::RedirectInAppend => self.current_io_type = IoType::InAppend,
                TokenKind::RedirectInTruncate => self.current_io_type = IoType::InTruncate,
                TokenKind::RedirectOutAppend => self.current_io_type = IoType::OutAppend,
                TokenKind::RedirectOutTruncate => self.current_io_type = IoType::OutTruncate,

                TokenKind::QuoteStart | TokenKind::QuoteEnd | TokenKind::Blank | TokenKind::Separator => (),
            }
        }

        match job_builder.build() {
            Ok(job) => clx.executable.add_job(job),
            Err(_) => (),
        };

        info!("{:#?}", &clx.executable);
        Ok(Response::Ok)
    }

    fn detokenize_to(&mut self, index: usize, clx: &mut Context) -> Result<Response, Error> {
        let total_len = clx.tokens.total_len();

        if total_len <= index {
            return Ok(Response::Skip);
        }

        let n = total_len - index;
        for _ in 0..n {
            self.remove(clx);
        }

        Ok(Response::Ok)
    }

    fn tokenize_from(&mut self, index: usize, clx: &mut Context) -> Result<Response, Error> {
        if index >= clx.line.len() {
            return Ok(Response::Skip);
        }
        let dirty_line = clx.line.get()[index..].to_string();
        for ch in dirty_line.chars() {
            self.add(clx, ch);
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
            self.add(clx, ch);
        }

        info!("Lexer tokens with alias: {:#?}", clx.tokens);
        Ok(Response::Ok)
    }

    fn add(&mut self, clx: &mut Context, ch: char) {
        if clx
            .tokens
            .last()
            .is_some_and(|token| token.clx().in_quote.is_some_and(|quote| quote != ch))
        {
            self.add_ambiguous(clx, ch);
            return;
        }

        match ch {
            // Job control
            ';' => self.add_separator(clx, ch),
            '&' => self.add_background_or_logical_and(clx, ch),
            '|' => self.add_pipe_or_logical_or(clx, ch),
            // Redirection
            '>' => self.add_redirect_out_append_or_truncate(clx, ch),
            '<' => self.add_redirect_in_append_or_truncate(clx, ch),
            // Quotes
            '\"' | '\'' => self.add_quote(clx, ch),
            // Other
            ' ' | '\t' => self.add_blank(clx, ch),
            ch => self.add_ambiguous(clx, ch),
        }
    }

    fn remove(&mut self, clx: &mut Context) {
        let Some(last_token) = clx.tokens.last_mut() else {
            return;
        };

        match *last_token.kind() {
            TokenKind::And => {
                warn!("Before pop and");
                let rm = clx.tokens.pop();
                warn!("Removed and: {:?}", rm);
                let replace_token = match clx.tokens.last() {
                    Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::Background, '&'),
                    None => Token::new_first(TokenKind::Background, '&'),
                };
                clx.line.mark_dirty_at(replace_token.clx().line_pos);
                clx.tokens.push(replace_token);
            }
            TokenKind::Or => {
                clx.tokens.pop();
                let replace_token = match clx.tokens.last() {
                    Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::Pipe, '|'),
                    None => Token::new_first(TokenKind::Pipe, '|'),
                };
                clx.line.mark_dirty_at(replace_token.clx().line_pos);
                clx.tokens.push(replace_token);
            }
            TokenKind::RedirectInAppend => {
                clx.tokens.pop();
                let replace_token = match clx.tokens.last() {
                    Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::RedirectInTruncate, '<'),
                    None => Token::new_first(TokenKind::RedirectInTruncate, '<'),
                };
                clx.line.mark_dirty_at(replace_token.clx().line_pos);
                clx.tokens.push(replace_token);
            }
            TokenKind::RedirectOutAppend => {
                clx.tokens.pop();
                let replace_token = match clx.tokens.last() {
                    Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::RedirectOutTruncate, '>'),
                    None => Token::new_first(TokenKind::RedirectOutTruncate, '>'),
                };
                clx.line.mark_dirty_at(replace_token.clx().line_pos);
                clx.tokens.push(replace_token);
            }
            _ => {
                match last_token.pop() {
                    Ok(_) => return,
                    Err(_) => clx.tokens.pop(),
                };
            }
        }
    }

    fn add_redirect_out_append_or_truncate(&mut self, clx: &mut Context, ch: char) {
        // If no token => create first token
        let Some(last_token) = clx.tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::RedirectOutTruncate, ch);
            clx.tokens.push(first_token);
            return;
        };

        // If last token is truncate => remove it and add append
        if *last_token.kind() == TokenKind::RedirectOutTruncate {
            clx.tokens.pop();
            let mut next_token = match clx.tokens.last() {
                Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::RedirectOutAppend, ch),
                None => Token::new_first(TokenKind::RedirectOutAppend, ch),
            };
            next_token.push(ch);
            clx.line.mark_dirty_at(next_token.clx().line_pos);
            clx.tokens.push(next_token);
            return;
        }

        // Else add next background token
        let next_token = Token::new_after(
            last_token.clx(),
            last_token.as_str(),
            TokenKind::RedirectOutTruncate,
            ch,
        );
        clx.tokens.push(next_token);
    }

    fn add_redirect_in_append_or_truncate(&mut self, clx: &mut Context, ch: char) {
        // If no token => create first token
        let Some(last_token) = clx.tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::RedirectInTruncate, ch);
            clx.tokens.push(first_token);
            return;
        };

        // If last token is truncate => remove it and add append
        if *last_token.kind() == TokenKind::RedirectInTruncate {
            clx.tokens.pop();
            let mut next_token = match clx.tokens.last() {
                Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::RedirectInAppend, ch),
                None => Token::new_first(TokenKind::RedirectInAppend, ch),
            };
            next_token.push(ch);
            clx.line.mark_dirty_at(next_token.clx().line_pos);
            clx.tokens.push(next_token);
            return;
        }

        // Else add next background token
        let next_token = Token::new_after(last_token.clx(), last_token.as_str(), TokenKind::RedirectInTruncate, ch);
        clx.tokens.push(next_token);
    }

    fn add_background_or_logical_and(&mut self, clx: &mut Context, ch: char) {
        // If no token => create first token
        let Some(last_token) = clx.tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Background, ch);
            clx.tokens.push(first_token);
            return;
        };

        // If last token is background => remove it and add logical and token
        if *last_token.kind() == TokenKind::Background {
            clx.tokens.pop();
            let mut next_token = match clx.tokens.last() {
                Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::And, ch),
                None => Token::new_first(TokenKind::And, ch),
            };
            next_token.push(ch);
            clx.line.mark_dirty_at(next_token.clx().line_pos);
            clx.tokens.push(next_token);
            return;
        }

        // Else add next background token
        let next_token = Token::new_after(last_token.clx(), last_token.as_str(), TokenKind::Background, ch);
        clx.tokens.push(next_token);
    }

    fn add_separator(&mut self, clx: &mut Context, ch: char) {
        // If no token => create first token
        let Some(last_token) = clx.tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Separator, ch);
            clx.tokens.push(first_token);
            return;
        };

        // Else add next separator token
        let next_token = Token::new_after(last_token.clx(), last_token.as_str(), TokenKind::Separator, ch);
        clx.tokens.push(next_token);
    }

    fn add_pipe_or_logical_or(&mut self, clx: &mut Context, ch: char) {
        // If no token => create first token
        let Some(last_token) = clx.tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Pipe, ch);
            clx.tokens.push(first_token);
            return;
        };

        // If last token is pipe => remove it and add logical or token
        if *last_token.kind() == TokenKind::Pipe {
            clx.tokens.pop();
            let mut next_token = match clx.tokens.last() {
                Some(token) => Token::new_after(token.clx(), token.as_str(), TokenKind::Or, ch),
                None => Token::new_first(TokenKind::Or, ch),
            };
            next_token.push(ch);
            clx.line.mark_dirty_at(next_token.clx().line_pos);
            clx.tokens.push(next_token);
            return;
        }

        // Else add next pipe token
        let next_token = Token::new_after(last_token.clx(), last_token.as_str(), TokenKind::Pipe, ch);
        clx.tokens.push(next_token);
    }

    fn add_ambiguous(&mut self, clx: &mut Context, ch: char) {
        // If no token => create first token
        let Some(last_token) = clx.tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Command, ch);
            clx.tokens.push(first_token);
            return;
        };

        // If last token is ambiguous => add to token
        if last_token.is_ambiguous() {
            last_token.push(ch);
            return;
        }

        // Else => create new ambiguous token
        let next_kind = if last_token.clx().require_file {
            TokenKind::File
        } else if last_token.has_segment_cmd() {
            TokenKind::Argument
        } else {
            TokenKind::Command
        };
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(prev_clx, last_token.as_str(), next_kind, ch);
        clx.tokens.push(next_token);
    }

    fn add_blank(&mut self, clx: &mut Context, ch: char) {
        // If no token => create first token
        let Some(last_token) = clx.tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::Blank, ch);
            clx.tokens.push(first_token);
            return;
        };

        // Else => Append blank token
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(prev_clx, last_token.as_str(), TokenKind::Blank, ch);
        clx.tokens.push(next_token);
    }

    fn add_quote(&mut self, clx: &mut Context, ch: char) {
        // If no token => create first token
        let Some(last_token) = clx.tokens.last_mut() else {
            let first_token = Token::new_first(TokenKind::QuoteStart, ch);
            clx.tokens.push(first_token);
            return;
        };

        // If in quote and char matches quote char => exit quote
        if last_token.is_in_quote_of(ch) {
            let prev_clx = last_token.clx();
            let next_token = Token::new_after(prev_clx, last_token.as_str(), TokenKind::QuoteEnd, ch);
            clx.tokens.push(next_token);
            return;
        }
        // If in quote with different char => add to in quote token
        else if last_token.is_in_quote() {
            last_token.push(ch);
            return;
        }

        // Else => Enter quote
        let prev_clx = last_token.clx();
        let next_token = Token::new_after(prev_clx, last_token.as_str(), TokenKind::QuoteStart, ch);
        clx.tokens.push(next_token);
    }
}
