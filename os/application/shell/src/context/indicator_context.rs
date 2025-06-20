use alloc::string::{String, ToString};

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
