use alloc::string::{String, ToString};

#[derive(Debug, Clone)]
pub struct IndicatorContext {
    indicator: String,
}

impl IndicatorContext {
    pub const fn new() -> Self {
        Self {
            indicator: String::new(),
        }
    }

    pub fn get(&self) -> &String {
        &self.indicator
    }

    pub fn set(&mut self, string: &str) {
        self.indicator = string.to_string();
    }

    pub fn len(&self) -> usize {
        self.indicator.len()
    }
}
