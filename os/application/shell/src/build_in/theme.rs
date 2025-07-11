use core::cell::RefCell;

use alloc::{rc::Rc, string::String, vec::Vec};
use terminal::{print, println};

use crate::context::theme_context::ThemeContext;

pub struct ThemeBuildIn {
    args: Vec<String>,
    theme_provider: Rc<RefCell<ThemeContext>>,
}

impl ThemeBuildIn {
    pub fn new(args: Vec<String>, theme_provider: Rc<RefCell<ThemeContext>>) -> Self {
        Self { args, theme_provider }
    }

    pub fn start(&self) -> Result<(), ()> {
        if self.args.is_empty() {
            return self.list_all_themes();
        }
        if self.args.len() != 1 {
            return self.usage();
        }

        let name = &self.args[0];

        if self.theme_provider.borrow_mut().set_current(name).is_ok() {
            return Ok(());
        }

        println!("Invalid argument: {} does not exist", name);
        self.list_all_themes()
    }

    fn list_all_themes(&self) -> Result<(), ()> {
        let theme = self.theme_provider.borrow();
        let theme_names = self.map_themes_to_str(&theme);

        if theme_names.is_empty() {
            println!("No theme available");
        } else {
            println!("Themes available: {}", theme_names);
        }

        Ok(())
    }

    fn usage(&self) -> Result<(), ()> {
        println!("Usage: theme NAME");
        Err(())
    }

    fn map_themes_to_str(&self, theme: &ThemeContext) -> String {
        theme
            .list_all()
            .iter()
            .map(|theme| theme.id)
            .collect::<Vec<&str>>()
            .join(", ")
    }
}
