use alloc::vec::Vec;
use terminal::DecodedKey;

use crate::lexer::lexer::Token;

pub enum VisualType {
    Indicator(char),
    Default(char),
    AutoCompleteHint(char),
}

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

    pub fn update(&mut self, data: T) {
        self.is_dirty = true;
        self.data = data;
    }

    pub fn get(&self) -> &T {
        &self.data
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn cleanup(&mut self) {
        self.is_dirty = false;
    }
}

pub struct Context {
    pub(crate) event: DecodedKey,
    pub(crate) line: ContextItem<Vec<char>>,
    pub(crate) cursor_position: ContextItem<usize>,
    pub(crate) visual_line: ContextItem<Vec<VisualType>>,
    pub(crate) tokens: ContextItem<Vec<Token>>,
}

impl Context {
    pub const fn new() -> Self {
        Self {
            event: DecodedKey::Unicode('\0'),
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
