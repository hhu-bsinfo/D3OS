use alloc::vec::Vec;
use globals::application::{APPLICATION_REGISTRY, Application};
use terminal::print;

pub struct AutoComplete {
    applications: Vec<Application>,
    current_index: usize,
    current_complete: &'static str,
}

impl AutoComplete {
    pub fn new() -> Self {
        Self {
            applications: Vec::from(APPLICATION_REGISTRY.applications),
            current_index: 0,
            current_complete: "",
        }
    }

    pub fn complete_command(&mut self, partial_command: &str) {
        self.clear_completion();
        self.current_complete = match self.find_next(|app| app.name.starts_with(partial_command)) {
            Some(app) => &app.name[partial_command.len()..],
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

    fn find_next<F>(&self, mut predicate: F) -> Option<&Application>
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
                return Some(application);
            }
        }
        None
    }
}
