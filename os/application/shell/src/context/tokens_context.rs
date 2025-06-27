use alloc::vec::Vec;

use crate::modules::lexer::token::{Token, TokenStatus};

#[derive(Debug, Clone)]
pub struct TokensContext {
    tokens: Vec<Token>,
    error_count: usize,
}

impl TokensContext {
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
            error_count: 0,
        }
    }

    pub fn reset(&mut self) {
        self.tokens.clear();
        self.error_count = 0;
    }

    pub fn get(&self) -> &Vec<Token> {
        &self.tokens
    }

    pub fn last(&self) -> Option<&Token> {
        self.tokens.last()
    }

    pub fn last_mut(&mut self) -> Option<&mut Token> {
        self.tokens.last_mut()
    }

    pub fn push(&mut self, token: Token) {
        if matches!(token.clx().status, TokenStatus::Error(_)) {
            self.error_count += 1;
        }
        self.tokens.push(token);
    }

    pub fn pop(&mut self) -> Option<Token> {
        let token = self.tokens.pop();
        if token
            .as_ref()
            .is_some_and(|token| matches!(token.clx().status, TokenStatus::Error(_)))
        {
            self.error_count -= 1;
        }
        token
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn is_error(&self) -> bool {
        self.error_count > 0
    }

    pub fn is_incomplete(&self) -> bool {
        match self.tokens.last() {
            Some(token) => token.clx().status == TokenStatus::Incomplete,
            None => false,
        }
    }

    pub fn find_last_command(&self) -> Option<&Token> {
        let last_token = match self.tokens.last() {
            Some(token) => token,
            None => return None,
        };
        let last_command_pos = match last_token.clx().cmd_pos {
            Some(pos) => pos,
            None => return None,
        };
        Some(&self.tokens[last_command_pos])
    }

    pub fn find_last_short_flag(&self) -> Option<&Token> {
        let last_token = match self.tokens.last() {
            Some(token) => token,
            None => return None,
        };
        let last_command_pos = match last_token.clx().short_flag_pos {
            Some(pos) => pos,
            None => return None,
        };
        Some(&self.tokens[last_command_pos])
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    // TODO add pos to token context, then if last last.pos + last.len else 0
    pub fn total_len(&self) -> usize {
        self.tokens.iter().map(|token| token.len()).sum()
    }
}
