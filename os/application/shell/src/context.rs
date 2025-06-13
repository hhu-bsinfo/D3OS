use core::cmp::min;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::{executable::Executable, service::lexer_service::Token};

#[derive(Debug, Clone, Default)]
pub struct LineContext {
    line: String,
    dirty_index: usize,
}

impl LineContext {
    pub fn new() -> Self {
        LineContext::default()
    }

    pub fn reset(&mut self) {
        *self = LineContext::default();
    }

    pub fn mark_clean(&mut self) {
        self.dirty_index = self.line.len();
    }

    pub fn mark_dirty_at(&mut self, index: usize) {
        self.dirty_index = min(self.dirty_index, index);
    }

    pub fn get(&self) -> &String {
        &self.line
    }

    pub fn get_dirty_part(&self) -> &str {
        &self.line[self.dirty_index..]
    }

    pub fn get_dirty_index(&self) -> usize {
        self.dirty_index
    }

    pub fn len(&self) -> usize {
        self.line.len()
    }

    pub fn push(&mut self, ch: char) {
        self.line.push(ch);
    }

    pub fn push_str(&mut self, string: &str) {
        self.line.push_str(string);
    }

    pub fn pop(&mut self) -> Option<char> {
        let ch = self.line.pop();
        if ch.is_some() {
            self.mark_dirty_at(self.line.len());
        }
        ch
    }

    pub fn insert(&mut self, index: usize, ch: char) {
        self.line.insert(index, ch);
        self.mark_dirty_at(index);
    }

    pub fn remove(&mut self, index: usize) {
        self.line.remove(index);
        self.mark_dirty_at(index);
    }
}

#[derive(Debug, Clone)]
pub struct IndicatorContext {
    indicator: String,
    is_dirty: bool,
}

impl IndicatorContext {
    pub fn new() -> Self {
        IndicatorContext::default()
    }

    pub fn get(&self) -> &String {
        &self.indicator
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn set(&mut self, string: &str) {
        self.indicator = string.to_string();
        self.is_dirty = true;
    }

    pub fn len(&self) -> usize {
        self.indicator.len()
    }

    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
    }
}

impl Default for IndicatorContext {
    fn default() -> Self {
        Self {
            indicator: String::default(),
            is_dirty: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Context {
    /// Current command line
    pub(crate) line: LineContext,
    /// Command line indicator
    pub(crate) indicator: IndicatorContext,
    /// Command line suggestion (Auto complete)
    pub(crate) suggestion: String,
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
    pub fn new() -> Self {
        Self {
            line: LineContext::new(),
            indicator: IndicatorContext::new(),
            suggestion: String::new(),
            is_suggestion_dirty: true,
            cursor_position: 0,
            tokens: Vec::new(),
            executable: None,
            is_autocompletion_active: false,
        }
    }

    /// Returns total line len including prefix and suffix
    pub fn total_line_len(&self) -> usize {
        self.indicator.len() + self.line.len() + self.suggestion.len()
    }

    pub fn is_cursor_at_end(&self) -> bool {
        self.cursor_position == self.line.len()
    }
}
