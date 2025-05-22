use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use globals::application::{APPLICATION_REGISTRY, Application};
use terminal::print;

pub struct AutoComplete {
    applications: Vec<Application>,
    current_index: usize,
    current_complete: &'static str,
    current_command: String,
}

impl AutoComplete {
    pub fn new() -> Self {
        Self {
            applications: Vec::from(APPLICATION_REGISTRY.applications),
            current_index: 0,
            current_complete: "",
            current_command: String::new(),
        }
    }

    pub fn complete_command(&mut self, partial_command: &str) {
        self.current_command = partial_command.to_string();
        self.current_index = 0;
        self.toggle_command();
    }

    pub fn toggle_command(&mut self) {
        self.clear_completion();
        let current_command = self.current_command.clone();
        self.current_complete = match self.find_next(|app| app.name.starts_with(&current_command)) {
            Some(app) => &app.name[self.current_command.len()..],
            None => "",
        };
        self.print_completion();
    }

    pub fn print_completion(&self) {
        if self.current_complete.is_empty() {
            return;
        }
        print!(
            "[38;2;100;100;100m{}[0m\x1b[{}D",
            self.current_complete,
            self.current_complete.len()
        );
    }

    pub fn clear_completion(&self) {
        if self.current_complete.is_empty() {
            return;
        }
        print!(
            "{}\x1b[{}D",
            " ".repeat(self.current_complete.len()),
            self.current_complete.len()
        );
    }

    fn find_next<F>(&mut self, mut predicate: F) -> Option<&Application>
    where
        F: FnMut(&Application) -> bool,
    {
        let length = self.applications.len();
        if length == 0 {
            return None;
        }

        for offset in 1..=length {
            let index = (self.current_index + offset) % length;
            let application = &self.applications[index];
            if predicate(application) {
                self.current_index = index;
                return Some(application);
            }
        }
        None
    }
}
