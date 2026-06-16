use alloc::string::{String, ToString};

#[derive(Debug, Clone)]
pub struct SuggestionContext {
    suggestion: String,
    is_dirty: bool,
    has_focus: bool,
}

impl SuggestionContext {
    pub fn new() -> Self {
        SuggestionContext::default()
    }

    pub fn reset(&mut self) {
        *self = SuggestionContext::default()
    }

    pub fn get(&self) -> &str {
        &self.suggestion
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn set(&mut self, string: &str) {
        self.suggestion = string.to_string();
        self.is_dirty = true;
    }

    pub fn len(&self) -> usize {
        self.suggestion.len()
    }

    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
    }

    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    pub fn focus(&mut self) {
        self.has_focus = true;
        self.is_dirty = true;
    }

    pub fn unfocus(&mut self) {
        self.has_focus = false;
        self.is_dirty = true;
    }

    pub fn is_empty(&self) -> bool {
        self.suggestion.is_empty()
    }
}

impl Default for SuggestionContext {
    fn default() -> Self {
        Self {
            suggestion: String::default(),
            is_dirty: true,
            has_focus: false,
        }
    }
}
