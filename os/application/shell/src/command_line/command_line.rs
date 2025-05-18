use alloc::{string::String, vec::Vec};
use terminal::print;

pub struct CommandLine {
    history: Vec<String>,
    current_line: Vec<char>,
    cursor_position: usize,
}

impl CommandLine {
    pub const fn new() -> Self {
        Self {
            history: Vec::new(),
            current_line: Vec::new(),
            cursor_position: 0,
        }
    }

    pub fn submit(&mut self) {
        self.history.push(self.current_line.iter().collect());
        self.current_line.clear();
        self.cursor_position = 0;
        print!("\n");
    }

    /// TODO docs: NOT FOR '\n', '\x08'
    pub fn add_char(&mut self, ch: char) -> Result<usize, ()> {
        self.current_line.insert(self.cursor_position, ch);
        self.cursor_position += 1;
        print!("{}", ch);
        Ok(self.cursor_position)
    }

    pub fn remove_before_cursor(&mut self) -> Result<usize, ()> {
        if self.cursor_position == 0 {
            return Err(());
        }

        self.cursor_position -= 1;
        self.current_line.remove(self.cursor_position);
        print!("\x1b[1D \x1b[1D");
        Ok(self.cursor_position)
    }
}
