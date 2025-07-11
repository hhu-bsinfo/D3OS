use alloc::vec::Vec;

use crate::modules::parser::token::{Token, TokenKind, TokenStatus};

#[derive(Debug, Clone)]
pub struct TokensContext {
    tokens: Vec<Token>,
    is_status_dirty: bool,
}

impl TokensContext {
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
            is_status_dirty: false,
        }
    }

    pub fn reset(&mut self) {
        self.tokens.clear();
        self.is_status_dirty = false;
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

    pub fn slice_at_line_index(&self, index: usize) -> &[Token] {
        let start_at = self
            .tokens
            .iter()
            .rposition(|token| token.clx().line_pos + token.len() <= index)
            .map_or(0, |i| i + 1);

        &self.tokens[start_at..]
    }

    pub fn push(&mut self, token: Token) {
        self.is_status_dirty |= if let Some(last) = self.tokens.last() {
            last.status() != token.status()
        } else {
            !token.status().is_valid()
        };

        self.tokens.push(token);
    }

    pub fn pop(&mut self) -> Option<Token> {
        let token = self.tokens.pop();

        self.is_status_dirty |= if let Some(last) = self.tokens.last() {
            token.as_ref().map_or(false, |tok| tok.status() != last.status())
        } else {
            token.as_ref().map_or(false, |tok| !tok.status().is_valid())
        };

        token
    }

    pub fn status(&self) -> &TokenStatus {
        if self.tokens.is_empty() {
            return &TokenStatus::Valid;
        }
        self.tokens.last().unwrap().status()
    }

    pub fn mark_status_clean(&mut self) {
        self.is_status_dirty = false;
    }

    pub fn is_status_dirty(&self) -> bool {
        self.is_status_dirty
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

    pub fn find_last_argument_in_segment(&self) -> Option<&Token> {
        for token in self.tokens.iter().rev() {
            if token.clx().cmd_pos.is_none() {
                return None;
            }
            if *token.kind() == TokenKind::Argument {
                return Some(token);
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn total_len(&self) -> usize {
        self.tokens.last().map_or(0, |t| t.clx().line_pos + t.len())
    }
}
