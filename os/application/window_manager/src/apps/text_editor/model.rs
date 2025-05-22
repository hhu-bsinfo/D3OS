use concurrent::thread::switch;
use terminal::{println, DecodedKey};
use text_buffer::TextBuffer;
use alloc::string::String;
use terminal::print;
use time::set_date;

enum EditMode {
    Normal,
    Insert,
}

pub struct Document<'b>{
    path: Option<String>,
    text_buffer: TextBuffer<'b>,
    caret: usize,
    edit_mode: EditMode,
}


impl <'b>Document<'b> {
    pub fn new(path: Option<String>, text_buffer: TextBuffer<'b>) -> Document<'b> {
        let length = text_buffer.len();
        Document {path: path, text_buffer: text_buffer, caret: length , edit_mode: EditMode::Insert}
    }
    pub fn text_buffer(&self) -> &TextBuffer {
        &self.text_buffer
    }
    pub fn caret(&self) -> usize {
        self.caret
    }
    fn next_line(&self, pos: usize) -> Option<usize> {
        let mut index = 0;
        while let Some(c) = self.text_buffer.get_char(pos+index){
            if c == '\n'{
                break;
            }
            index +=1;
        }
        if self.text_buffer.get_char(pos+index+1).is_some() {
            return Some(pos+index+1);
        }
        None
    } 
    fn prev_line(&self, pos: usize) -> Option<usize> {
        let mut index = 0;
        while let Some(c) = self.text_buffer.get_char(pos-index){
            if c == '\n'{
                break;
            }
            index +=1;
            if pos < index {
                break;
            }
        }
        if self.text_buffer.get_char(pos-index+1).is_some() {
            return Some(pos-index+1);
        }
        None
    }

    fn move_cursor_down(&mut self){
        let text = self.text_buffer.to_string();
        let prev_line = self.prev_line(self.caret).unwrap_or(self.caret);
        let origin_len = self.caret - prev_line;
        let next_line = self.next_line(self.caret).unwrap_or(self.caret);
        self.caret = next_line + origin_len; 
    }

    fn move_cursor_up(&mut self){
        let text = self.text_buffer.to_string();
        let prev_line = self.prev_line(self.caret).unwrap_or(self.caret);
        let origin_len = self.caret - prev_line;
        let prev_prev_line = self.prev_line(prev_line-2).unwrap_or(self.caret);
        self.caret = prev_prev_line + origin_len; 
    }

    fn update_insert(&mut self, k: DecodedKey) {
        //delete
        match k {
            // delete
            DecodedKey::Unicode('\x08') => {
                self.text_buffer.delete(self.caret-1);
                self.caret -=1;
            }, 
            // esc
            DecodedKey::Unicode('\x1B') => {
                self.edit_mode = EditMode::Normal;
            }
            DecodedKey::Unicode(ch) => {
                self.text_buffer.insert(self.caret, ch);
                self.caret+=1;
            }
            DecodedKey::RawKey(terminal::KeyCode::ArrowLeft) => self.caret -=1,
            DecodedKey::RawKey(terminal::KeyCode::ArrowRight) => self.caret +=1,
            DecodedKey::RawKey(terminal::KeyCode::ArrowUp) => self.move_cursor_up(),
            DecodedKey::RawKey(terminal::KeyCode::ArrowDown) => self.move_cursor_down(),
            DecodedKey::RawKey(_) => todo!()
        }
    }

    fn update_normal(&mut self, k: DecodedKey) {
        // funktioniert irgendwie nicht
        println!("Ausgabe: {}",self.text_buffer.to_string());
        match k {
            DecodedKey::Unicode('h') => self.caret -=1,
            DecodedKey::Unicode('l') => self.caret +=1,
            DecodedKey::Unicode('j') => self.move_cursor_down(),
            DecodedKey::Unicode('k') => self.move_cursor_up(),
            DecodedKey::Unicode('i') => self.edit_mode = EditMode::Insert,
            DecodedKey::RawKey(terminal::KeyCode::ArrowLeft) => self.caret -=1,
            DecodedKey::RawKey(terminal::KeyCode::ArrowRight) => self.caret +=1,
            DecodedKey::RawKey(terminal::KeyCode::ArrowUp) => self.move_cursor_up(),
            DecodedKey::RawKey(terminal::KeyCode::ArrowDown) => self.move_cursor_down(),
            _ => (),
        }
        if self.caret > self.text_buffer.len() {
            self.caret = self.text_buffer.len();
        }
    }

    pub fn update(&mut self, k: DecodedKey) {
        match self.edit_mode {
            EditMode::Insert => self.update_insert(k),
            EditMode::Normal => self.update_normal(k),
        }
        
    }
}