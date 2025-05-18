use alloc::{string::String, vec::Vec};
use logger::info;
use terminal::print;

pub struct CommandLine {
    history: Vec<String>,
    current_line: String,
    cursor_position: usize,
    history_position: isize,
}

impl CommandLine {
    pub const fn new() -> Self {
        Self {
            history: Vec::new(),
            current_line: String::new(),
            cursor_position: 0,
            history_position: -1,
        }
    }

    pub fn submit(&mut self) {
        self.history.push(self.current_line.clone());
        self.history_position = -1;
        self.current_line.clear();
        self.cursor_position = 0;
        print!("\n");
    }

    pub fn create_new_line(&self) {
        print!("⮞ ");
    }

    /// TODO docs: NOT FOR '\n', '\x08'
    pub fn add_char(&mut self, ch: char) -> Result<String, ()> {
        self.current_line.insert(self.cursor_position, ch);
        let line_since_cursor = self.current_line.get(self.cursor_position..).unwrap();
        match line_since_cursor.len() - 1 {
            0 => print!("{}", line_since_cursor), // Note: \x1b[0D will still do \x1b[1D
            cursor_offset => print!("{}\x1b[{}D", line_since_cursor, cursor_offset),
        }
        self.cursor_position += 1;
        Ok(self.current_line.clone())
    }

    pub fn remove_at_cursor(&mut self) -> Result<String, ()> {
        if self.cursor_position == self.current_line.len() {
            return Err(());
        }

        self.current_line.remove(self.cursor_position);
        let line_since_cursor = self.current_line.get(self.cursor_position..).unwrap();
        let cursor_offset = line_since_cursor.len() + 1; // line_since_cursor.len() + 1 (we added whitespace to remove trailing char)
        print!("{} \x1b[{}D", line_since_cursor, cursor_offset);
        Ok(self.current_line.clone())
    }

    pub fn remove_before_cursor(&mut self) -> Result<String, ()> {
        if self.cursor_position == 0 {
            return Err(());
        }

        self.cursor_position -= 1;
        self.current_line.remove(self.cursor_position);
        let line_since_cursor = self.current_line.get(self.cursor_position..).unwrap();
        let cursor_offset = line_since_cursor.len() + 1; // line_since_cursor.len() - 1 + (1 + 1) (we moved cursor already & we added whitespace to remove trailing char)
        print!("\x1b[1D{} \x1b[{}D", line_since_cursor, cursor_offset);
        Ok(self.current_line.clone())
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

    pub fn move_history_up(&mut self) -> Result<String, ()> {
        if self.history_position == self.history.len() as isize - 1 {
            return Err(());
        }
        Ok(self.move_history(1))
    }

    pub fn move_history_down(&mut self) -> Result<String, ()> {
        if self.history_position <= -1 {
            return Err(());
        }
        if self.history_position == 0 {
            self.history_position = -1;
            self.clear_line();
            return Ok(self.current_line.clone());
        }
        Ok(self.move_history(-1))
    }

    fn move_history(&mut self, step: isize) -> String {
        self.history_position += step;
        let history = self.history.as_slice()[self.history_position as usize].clone();
        self.clear_line();
        print!("{}", history);
        self.cursor_position = history.len();
        self.current_line = history;
        info!("Current history position: {}", self.history_position);
        self.current_line.clone()
    }

    pub fn clear_line(&mut self) {
        match self.cursor_position {
            0 => print!("\x1b[0K \x1b[1D"), // Note: \x1b[0D will still do \x1b[1D
            offset => print!("\x1b[{}D\x1b[0K \x1b[1D", offset),
        };
        self.cursor_position = 0;
        self.current_line.clear();
    }
}
