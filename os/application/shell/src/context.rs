use alloc::vec::Vec;

use crate::service::lexer_service::Token;

#[derive(Debug, Clone)]
pub struct Context {
    /// Current command line
    pub(crate) line: Vec<char>,
    /// Prefix for command line (Indicator)
    pub(crate) line_prefix: Vec<char>,
    /// Suffix for command line (Auto complete)
    pub(crate) line_suffix: Vec<char>,
    /// Tells a service to skip validation until reached (INCLUDING PREFIX AND SUFFIX)
    pub(crate) dirty_offset: usize,
    /// Current cursor position (INCLUDING PREFIX AND SUFFIX)
    pub(crate) cursor_position: usize,
    /// Generated tokens based on line (does not contain prefix and suffix)
    pub(crate) tokens: Vec<Token>,
}

impl Context {
    pub const fn new() -> Self {
        Self {
            line: Vec::new(),
            line_prefix: Vec::new(),
            line_suffix: Vec::new(),
            dirty_offset: 0,
            cursor_position: 0,
            tokens: Vec::new(),
        }
    }

    /// Returns index, at which item line_prefix is dirty
    pub fn get_dirty_line_prefix_offset(&self) -> usize {
        self.dirty_offset.min(self.line_prefix.len())
    }

    /// Returns index, at which item line is dirty
    pub fn get_dirty_line_offset(&self) -> usize {
        self.dirty_offset.min(self.line.len())
    }

    /// Returns index, at which item line_suffix is dirty
    pub fn get_dirty_line_suffix_offset(&self) -> usize {
        self.dirty_offset.min(self.line_suffix.len())
    }

    /// Set dirty offset to index relative to line_prefix
    pub fn set_dirty_offset_from_line_prefix(&mut self, index: usize) {
        self.dirty_offset = index;
    }

    /// Set dirty offset to index relative to line
    pub fn set_dirty_offset_from_line(&mut self, index: usize) {
        let offset = self.line_prefix.len();
        self.dirty_offset = offset + index;
    }

    /// Set dirty offset to index relative to line_suffix
    pub fn set_dirty_offset_from_line_suffix(&mut self, index: usize) {
        let offset = self.line_prefix.len() + self.line.len();
        self.dirty_offset = offset + index;
    }

    /// Returns total line len including prefix and suffix
    pub fn total_line_len(&self) -> usize {
        self.line_prefix.len() + self.line.len() + self.line_suffix.len()
    }

    /// Returns the length of tokens, including inner lengths
    pub fn inner_tokens_len(&self) -> usize {
        self.tokens
            .iter()
            .map(|token| match token {
                Token::Command(s) => s.len(),
                Token::Argument(s) => s.len(),
                _ => 1,
            })
            .sum()
    }
}
