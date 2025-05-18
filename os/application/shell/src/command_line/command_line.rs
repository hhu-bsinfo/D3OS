use alloc::{string::String, vec::Vec};
use terminal::print;

pub struct CommandLine {
    history: Vec<String>,
    current_line: String,
    cursor_position: usize,
}

impl CommandLine {
    pub const fn new() -> Self {
        Self {
            history: Vec::new(),
            current_line: String::new(),
            cursor_position: 0,
        }
    }

    pub fn submit(&mut self) {
        self.history.push(self.current_line.clone());
        self.current_line.clear();
        self.cursor_position = 0;
        print!("\n");
    }

    /// TODO docs: NOT FOR '\n', '\x08'
    pub fn add_char(&mut self, ch: char) -> Result<usize, ()> {
        self.current_line.insert(self.cursor_position, ch);
        let line_since_cursor = self.current_line.get(self.cursor_position..).unwrap();
        match line_since_cursor.len() - 1 {
            0 => print!("{}", line_since_cursor), // Note: \x1b[0D will still do \x1b[1D
            cursor_offset => print!("{}\x1b[{}D", line_since_cursor, cursor_offset),
        }
        self.cursor_position += 1;
        Ok(self.cursor_position)
    }

    pub fn remove_before_cursor(&mut self) -> Result<usize, ()> {
        if self.cursor_position == 0 {
            return Err(());
        }

        self.cursor_position -= 1;
        self.current_line.remove(self.cursor_position);
        let line_since_cursor = self.current_line.get(self.cursor_position..).unwrap();
        let cursor_offset = line_since_cursor.len() + 1; // line_since_cursor.len() - 1 + (1 + 1) (we moved cursor already & we added whitespace to remove trailing char)
        print!("\x1b[1D{} \x1b[{}D", line_since_cursor, cursor_offset);
        Ok(self.cursor_position)
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position == 0 {
            return;
        }

        self.cursor_position -= 1;
        print!("\x1b[1D");
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_position >= self.current_line.len() {
            return;
        }

        self.cursor_position += 1;
        print!("\x1b[1C");
    }
}
