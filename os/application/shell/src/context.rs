use core::cmp::min;

use alloc::{string::String, vec::Vec};

use crate::{executable::Executable, service::lexer_service::Token};

#[derive(Debug, Clone)]
pub struct Context {
    /// Current command line
    pub(crate) line: String,
    /// Command line indicator
    pub(crate) indicator: String,
    /// Command line suggestion (Auto complete)
    pub(crate) suggestion: String,
    /// Tells a service to skip line validation until given index
    pub(crate) line_dirty_at: usize,
    /// Tells a service to validate indicator or not
    pub(crate) is_indicator_dirty: bool,
    /// Tells a service to validate suggestion or not
    pub(crate) is_suggestion_dirty: bool,
    /// Current cursor position
    pub(crate) cursor_position: usize,
    /// Generated tokens based on line
    pub(crate) tokens: Vec<Token>,
    /// Generated executable based on tokens
    pub(crate) executable: Option<Executable>,
    /// Tells drawer to visualize auto completion
    pub(crate) is_autocompletion_active: bool,
}

impl Context {
    pub const fn new() -> Self {
        Self {
            line: String::new(),
            indicator: String::new(),
            suggestion: String::new(),
            line_dirty_at: 0,
            is_indicator_dirty: true,
            is_suggestion_dirty: true,
            cursor_position: 0,
            tokens: Vec::new(),
            executable: None,
            is_autocompletion_active: false,
        }
    }

    pub fn set_dirty_line_index(&mut self, index: usize) {
        self.line_dirty_at = min(self.line_dirty_at, index);
    }

    /// Returns total line len including prefix and suffix
    pub fn total_line_len(&self) -> usize {
        self.indicator.len() + self.line.len() + self.suggestion.len()
    }

    pub fn is_cursor_at_end(&self) -> bool {
        self.cursor_position == self.line.len()
    }
}
