use concurrent::thread::switch;
use terminal::{println, DecodedKey};
use text_buffer::TextBuffer;
use alloc::string::String;
use terminal::print;

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
            DecodedKey::RawKey(_) => todo!()
        }
    }

    fn update_normal(&mut self, k: DecodedKey) {
        // funktioniert irgendwie nicht
        println!("Ausgabe: {}",self.text_buffer.to_string());
        match k {
            DecodedKey::Unicode('h') => self.caret -=1,
            DecodedKey::Unicode('l') => self.caret +=1,
            DecodedKey::Unicode('i') => self.edit_mode = EditMode::Insert,
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