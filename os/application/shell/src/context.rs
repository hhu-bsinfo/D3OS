use core::cmp::min;

use alloc::{string::String, vec::Vec};

use crate::{executable::Executable, service::lexer_service::Token};

#[derive(Debug, Clone)]
pub struct Context {
    /// Current command line
    pub(crate) line: String,
    /// Prefix for command line (Indicator)
    pub(crate) line_prefix: String,
    /// Suffix for command line (Auto complete)
    pub(crate) line_suffix: String,
    /// Tells a service to skip validation until reached (INCLUDING PREFIX AND SUFFIX)
    pub(crate) dirty_offset: usize,
    /// Current cursor position
    pub(crate) cursor_position: usize,
    /// Generated tokens based on line
    pub(crate) tokens: Vec<Token>,
    /// Generated executable based on tokens
    pub(crate) executable: Option<Executable>,
    /// Tells drawer to visualize auto completion
    pub(crate) is_autocomplete_active: bool,
}

impl Context {
    pub const fn new() -> Self {
        Self {
            line: String::new(),
            line_prefix: String::new(),
            line_suffix: String::new(),
            dirty_offset: 0,
            cursor_position: 0,
            tokens: Vec::new(),
            executable: None,
            is_autocomplete_active: false,
        }
    }

    /// Returns index, at which item line_prefix is dirty
    pub fn get_dirty_line_prefix_offset(&self) -> usize {
        clamp_index(self.dirty_offset, 0, self.line_prefix.len())
    }

    /// Returns index, at which item line is dirty
    pub fn get_dirty_line_offset(&self) -> usize {
        let base = self.line_prefix.len();
        clamp_index(self.dirty_offset, base, self.line.len())
    }

    /// Returns index, at which item line_suffix is dirty
    pub fn get_dirty_line_suffix_offset(&self) -> usize {
        let base = self.line_prefix.len() + self.line.len();
        clamp_index(self.dirty_offset, base, self.line_suffix.len())
    }

    /// Set dirty offset to index relative to line_prefix
    pub fn set_dirty_offset_from_line_prefix(&mut self, index: usize) {
        self.dirty_offset = index;
    }

    /// Set dirty offset to index relative to line
    pub fn set_dirty_offset_from_line(&mut self, index: usize) {
        let offset = self.line_prefix.len();
        self.dirty_offset = min(self.dirty_offset, offset + index);
    }

    /// Set dirty offset to index relative to line_suffix
    pub fn set_dirty_offset_from_line_suffix(&mut self, index: usize) {
        let offset = self.line_prefix.len() + self.line.len();
        self.dirty_offset = min(self.dirty_offset, offset + index);
    }

    /// Returns total line len including prefix and suffix
    pub fn total_line_len(&self) -> usize {
        self.line_prefix.len() + self.line.len() + self.line_suffix.len()
    }

    // TODO MOVE TO TOKEN IMPL
    /// Returns the length of tokens, including inner lengths
    pub fn inner_tokens_len(&self) -> usize {
        self.tokens
            .iter()
            .map(|token| match token {
                Token::Command(_clx, s) => s.len(),
                Token::Argument(_clx, s) => s.len(),
                _ => 1,
            })
            .sum()
    }

    pub fn is_cursor_at_end(&self) -> bool {
        self.cursor_position == self.line.len()
    }
}

fn clamp_index(i: usize, base: usize, len: usize) -> usize {
    i.saturating_sub(base).min(len)
}
