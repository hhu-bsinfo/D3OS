use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use globals::application::{APPLICATION_REGISTRY, Application};
use logger::info;
use terminal::print;

#[derive(Debug)]
enum SelectionState {
    Unselected,
    Selected,
}

/// Cursor position relative to completion
#[derive(Debug, PartialEq)]
enum CursorPosition {
    Left,
    Right,
}

#[derive(Debug)]
pub struct AutoComplete {
    applications: Vec<Application>,
    current_index: usize,
    last_completion: &'static str,
    completion: &'static str,
    current_command: String,
    selection_state: SelectionState,
    cursor_position: CursorPosition,
}

impl AutoComplete {
    pub fn new() -> Self {
        Self {
            applications: Vec::from(APPLICATION_REGISTRY.applications),
            current_index: 0,
            last_completion: "",
            completion: "",
            current_command: String::new(),
            selection_state: SelectionState::Unselected,
            cursor_position: CursorPosition::Left,
        }
    }

    pub fn complete_command(&mut self, partial_command: &str) {
        self.current_command = partial_command.to_string();
        self.current_index = 0;
        self.cycle_command();
        self.print(CursorPosition::Left);
    }

    pub fn select_or_cycle(&mut self) {
        if self.completion.is_empty() {
            self.cycle_command();
        }
        match self.selection_state {
            SelectionState::Unselected => self.select_completion(),
            SelectionState::Selected => self.cycle_command(),
        };
        self.print(CursorPosition::Right);
    }

    pub fn flush(&mut self) -> &'static str {
        info!("{:?}", self);
        let completion = self.completion.clone();
        self.last_completion = self.completion;
        self.cleanup_last_completion();
        self.reset();
        completion
    }

    fn reset(&mut self) {
        self.current_index = 0;
        self.last_completion = "";
        self.completion = "";
        self.current_command.clear();
        self.selection_state = SelectionState::Unselected;
        self.cursor_position = CursorPosition::Left;
    }

    pub fn remove_command(&mut self) {
        self.selection_state = SelectionState::Unselected;
        self.last_completion = self.completion.clone();
        self.completion = "";
        self.cleanup_last_completion();
    }

    fn select_completion(&mut self) {
        if self.completion.is_empty() {
            return;
        }
        self.selection_state = SelectionState::Selected;
    }

    fn print(&mut self, cursor_target: CursorPosition) {
        self.cleanup_last_completion();
        print!("[38;2;100;100;100m{}[0m", self.completion);

        if cursor_target == CursorPosition::Left && !self.completion.is_empty() {
            print!("\x1b[{}D", self.completion.len());
        }

        self.cursor_position = cursor_target;
    }

    pub fn cleanup_last_completion(&self) {
        if self.last_completion.is_empty() {
            return;
        }
        let len = self.last_completion.len();
        if self.cursor_position == CursorPosition::Right {
            print!("\x1b[{}D", len);
        }
        print!("{}\x1b[{}D", " ".repeat(len), len);
    }

    fn cycle_command(&mut self) {
        let current_command = self.current_command.clone();
        self.last_completion = self.completion.clone();
        self.completion = match self.find_next(|app| app.name.starts_with(&current_command)) {
            Some(app) => &app.name[self.current_command.len()..],
            None => "",
        };
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
