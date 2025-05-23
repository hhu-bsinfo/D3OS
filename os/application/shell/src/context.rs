use alloc::vec::Vec;

use crate::lexer::lexer::Token;

#[derive(Debug, PartialEq)]
pub enum VisualType {
    Indicator(char),
    Default(char),
    AutoCompleteHint(char),
}

#[derive(Debug)]
pub struct ContextItem<T> {
    is_dirty: bool,
    data: T,
}

impl<T> ContextItem<T> {
    pub const fn new(data: T) -> Self {
        Self {
            is_dirty: true,
            data,
        }
    }

    pub fn set(&mut self, data: T) {
        self.is_dirty = true;
        self.data = data;
    }

    pub fn get(&self) -> &T {
        &self.data
    }

    /// When modifying existing data instead of overwriting
    pub fn dirty_mut(&mut self) -> &mut T {
        self.is_dirty = true;
        &mut self.data
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn cleanup(&mut self) {
        self.is_dirty = false;
    }
}

#[derive(Debug)]
pub struct Context {
    pub(crate) line: ContextItem<Vec<char>>,
    pub(crate) cursor_position: ContextItem<usize>,
    pub(crate) visual_line: ContextItem<Vec<VisualType>>,
    pub(crate) tokens: ContextItem<Vec<Token>>,
}

impl Context {
    pub const fn new() -> Self {
        Self {
            line: ContextItem::new(Vec::new()),
            cursor_position: ContextItem::new(0),
            visual_line: ContextItem::new(Vec::new()),
            tokens: ContextItem::new(Vec::new()),
        }
    }

    pub fn cleanup(&mut self) {
        self.line.cleanup();
        self.cursor_position.cleanup();
        self.visual_line.cleanup();
        self.tokens.cleanup();
    }
}
