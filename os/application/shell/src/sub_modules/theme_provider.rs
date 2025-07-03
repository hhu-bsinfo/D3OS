use globals::theme::{THEME_REGISTRY, Theme};

#[derive(Debug)]
pub struct ThemeProvider {}

impl ThemeProvider {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get(&self) -> &'static Theme {
        THEME_REGISTRY.default
    }
}
