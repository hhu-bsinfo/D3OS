use alloc::vec::Vec;

use crate::modules::lexer::Token;

#[derive(Debug, Clone, Default)]
pub struct TokensContext {
    tokens: Vec<Token>,
}

impl TokensContext {
    pub fn new() -> Self {
        TokensContext::default()
    }

    pub fn reset(&mut self) {
        *self = TokensContext::default()
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
        self.tokens.push(token);
    }

    pub fn pop(&mut self) -> Option<Token> {
        self.tokens.pop()
    }

    pub fn find_last_command(&self) -> Option<&Token> {
        let last_token = match self.tokens.last() {
            Some(token) => token,
            None => return None,
        };
        let last_command_pos = match last_token.context().assigned_command_pos {
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
        let last_command_pos = match last_token.context().assigned_short_flag_pos {
            Some(pos) => pos,
            None => return None,
        };
        Some(&self.tokens[last_command_pos])
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn total_len(&self) -> usize {
        self.tokens
            .iter()
            .map(|token| match token {
                Token::Command(_clx, s) => s.len(),
                Token::Argument(_clx, s) => s.len(),
                _ => 1,
            })
            .sum()
    }
}
