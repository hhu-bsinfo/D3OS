use core::cell::RefCell;

use alloc::{rc::Rc, string::String, vec::Vec};
use terminal::println;

use crate::{built_in::built_in::BuiltIn, context::theme_context::ThemeContext};

pub struct ThemeBuiltIn {
    theme_provider: Rc<RefCell<ThemeContext>>,
}

impl BuiltIn for ThemeBuiltIn {
    fn namespace(&self) -> &'static str {
        "theme"
    }

    fn run(&mut self, args: &[&str]) -> isize {
        if args.is_empty() {
            return self.list_all_themes();
        }
        if args.len() != 1 {
            Self::print_usage();
            return -1;
        }

        let name = args.get(0).unwrap();
        if self.theme_provider.borrow_mut().set_current(name).is_err() {
            println!("Invalid argument: {} does not exist", name);
            self.list_all_themes();
            return -1;
        }

        0
    }
}

impl ThemeBuiltIn {
    pub fn new(theme_provider: Rc<RefCell<ThemeContext>>) -> Self {
        Self { theme_provider }
    }

    fn list_all_themes(&self) -> isize {
        let theme = self.theme_provider.borrow();
        let theme_names = Self::map_themes_to_str(&theme);
        if theme_names.is_empty() {
            println!("No theme available");
        } else {
            println!("Themes available: {}", theme_names);
        }
        0
    }

    fn print_usage() {
        println!("Usage: theme NAME");
    }

    fn map_themes_to_str(theme: &ThemeContext) -> String {
        theme
            .list_all()
            .iter()
            .map(|theme| theme.id)
            .collect::<Vec<&str>>()
            .join(", ")
    }
}
