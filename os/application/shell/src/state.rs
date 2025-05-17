use alloc::string::String;
use logger::info;

use crate::parser::command_line::CommandLine;

pub struct State {
    pub(crate) current_line: String,
    pub(crate) read_char: Option<char>,
    pub(crate) submit: bool,
    pub(crate) command_line: Option<CommandLine>,
}

impl State {
    pub const fn new() -> Self {
        Self {
            current_line: String::new(),
            read_char: None,
            submit: false,
            command_line: None,
        }
    }

    pub fn clear(&mut self) {
        info!(
            "Before cleanup current line: {}, read_char: {:?}, submit: {}, command_line: {:?}",
            self.current_line, self.read_char, self.submit, self.command_line
        );
        self.current_line.clear();
        self.read_char = None;
        self.submit = false;
        self.command_line = None;
        info!(
            "After cleanup current line: {}, read_char: {:?}, submit: {}, command_line: {:?}",
            self.current_line, self.read_char, self.submit, self.command_line
        );
    }
}
