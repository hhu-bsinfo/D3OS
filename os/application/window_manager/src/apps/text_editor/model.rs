use alloc::string::String;
use concurrent::thread::switch;
use logger::{error, warn};
use terminal::print;
use terminal::{println, DecodedKey};
use text_buffer::TextBuffer;
use time::set_date;

use super::meassages::{Message, ViewMessage};
use super::view::View;
use super::TextEditorConfig;

enum EditMode {
    Normal,
    Insert,
}

pub struct Document<'b> {
    path: Option<String>,
    text_buffer: TextBuffer<'b>,
    caret: usize,
    edit_mode: EditMode,
    config: TextEditorConfig,
    current_view: View,
    scroll_offset: u32,
}

impl<'b> Document<'b> {
    pub fn new(path: Option<String>, text_buffer: TextBuffer<'b>, config: TextEditorConfig) -> Document<'b> {
        let length = text_buffer.len();
        Document {
            path: path,
            text_buffer: text_buffer,
            caret: 0,
            edit_mode: EditMode::Insert,
            config: config,
            current_view: config.simple_view,
            scroll_offset: 0,
        }
    }
    pub fn text_buffer(&self) -> &TextBuffer {
        &self.text_buffer
    }
    pub fn scroll_offset(&self) -> u32 {
        self.scroll_offset
    }
    pub fn caret(&self) -> usize {
        self.caret
    }
    fn next_line(&self, pos: usize) -> Option<usize> {
        let mut index = 0;
        while let Some(c) = self.text_buffer.get_char(pos + index) {
            if c == '\n' {
                break;
            }
            index += 1;
        }
        if self.text_buffer.get_char(pos + index + 1).is_some() {
            return Some(pos + index + 1);
        }
        None
    }
    fn prev_line(&self, pos: usize) -> Option<usize> {
        let mut index = 0;
        while let Some(c) = self.text_buffer.get_char(pos - index) {
            if c == '\n' {
                break;
            }
            index += 1;
            if pos < index {
                break;
            }
        }
        if self.text_buffer.get_char(pos - index + 1).is_some() {
            return Some(pos - index + 1);
        }
        None
    }

    fn move_cursor_down(&mut self) {
        let text = self.text_buffer.to_string();
        let prev_line = self.prev_line(self.caret).unwrap_or(self.caret);
        let origin_len = self.caret - prev_line;
        let next_line = self.next_line(self.caret).unwrap_or(self.caret);
        self.caret = next_line + origin_len;
    }

    fn move_cursor_up(&mut self) {
        let text = self.text_buffer.to_string();
        let prev_line = self.prev_line(self.caret).unwrap_or(self.caret);
        let origin_len = self.caret - prev_line;
        let prev_prev_line = self.prev_line(prev_line - 2).unwrap_or(self.caret);
        self.caret = prev_prev_line + origin_len;
    }

    fn update_insert(&mut self, k: DecodedKey) -> View{
        //delete
        match k {
            // delete
            DecodedKey::Unicode('\x08') => {
                self.text_buffer.delete(self.caret - 1);
                self.caret -= 1;
            }
            // esc
            DecodedKey::Unicode('\x1B') => {
                self.edit_mode = EditMode::Normal;
            }
            DecodedKey::Unicode(ch) => {
                self.text_buffer.insert(self.caret, ch);
                self.caret += 1;
            }
            DecodedKey::RawKey(terminal::KeyCode::ArrowLeft) => self.caret -= 1,
            DecodedKey::RawKey(terminal::KeyCode::ArrowRight) => self.caret += 1,
            DecodedKey::RawKey(terminal::KeyCode::ArrowUp) => self.move_cursor_up(),
            DecodedKey::RawKey(terminal::KeyCode::ArrowDown) => self.move_cursor_down(),
            DecodedKey::RawKey(k) => warn!("TextEditor can't process input: {:?}", k),
        }
        self.current_view
    }

    fn update_normal(&mut self, k: DecodedKey) -> View {
        // funktioniert irgendwie nicht
        println!("Ausgabe: {}", self.text_buffer.to_string());
        match k {
            DecodedKey::Unicode('u') => {
                self.text_buffer.undo();
            }
            DecodedKey::Unicode('r') => {
                self.text_buffer.redo();
            }
            DecodedKey::Unicode('h') => self.caret -= 1,
            DecodedKey::Unicode('l') => self.caret += 1,
            DecodedKey::Unicode('j') => self.move_cursor_down(),
            DecodedKey::Unicode('k') => self.move_cursor_up(),
            DecodedKey::Unicode('i') => self.edit_mode = EditMode::Insert,
            DecodedKey::Unicode('n') => self.current_view = self.config.simple_view,
            DecodedKey::Unicode('m') => self.current_view = self.config.markdown_view,
            DecodedKey::RawKey(terminal::KeyCode::ArrowLeft) => self.caret -= 1,
            DecodedKey::RawKey(terminal::KeyCode::ArrowRight) => self.caret += 1,
            DecodedKey::RawKey(terminal::KeyCode::ArrowUp) => self.move_cursor_up(),
            DecodedKey::RawKey(terminal::KeyCode::ArrowDown) => self.move_cursor_down(),
            _ => (),
        }
        if self.caret > self.text_buffer.len() {
            self.caret = self.text_buffer.len();
        }
        self.current_view
    }

    fn scroll(&mut self, msg: ViewMessage) -> View {
        match msg {
            ViewMessage::ScrollDown(v) => self.scroll_offset += v,
            ViewMessage::ScrollUp(v) => self.scroll_offset = self.scroll_offset.checked_sub(v).unwrap_or(0),
        }
        self.current_view
    }

    pub fn update(&mut self, m: Message) -> View {
        match m {
            Message::ViewMessage(msg) => self.scroll(msg),
            Message::DecodedKey(k) => {
                match self.edit_mode {
                    EditMode::Insert => self.update_insert(k),
                    EditMode::Normal => self.update_normal(k),
                }
            }
        }
    }
}
