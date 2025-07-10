use globals::theme::{THEME_REGISTRY, Theme};

#[derive(Debug)]
pub struct ThemeProvider {
    current: &'static Theme,
}

impl ThemeProvider {
    pub fn new() -> Self {
        Self {
            current: THEME_REGISTRY.default,
        }
    }

    pub fn get_current(&self) -> &'static Theme {
        self.current
    }

    pub fn set_current(&mut self, id: &str) -> Result<(), ()> {
        let Some(theme) = THEME_REGISTRY.themes.iter().find(|theme| theme.id == id) else {
            return Err(());
        };
        self.current = theme;
        Ok(())
    }

    pub fn list_all(&self) -> &'static [Theme] {
        THEME_REGISTRY.themes
    }
}
