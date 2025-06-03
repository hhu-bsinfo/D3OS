use alloc::string::String;
use graphic::color::Color;
use logger::warn;
use terminal::print;
use terminal::{println, DecodedKey};
use text_buffer::TextBuffer;

use super::font::Font;
use super::meassages::{Message, ViewMessage};
use super::view::View;
use super::TextEditorConfig;

enum EditMode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy)]
pub enum ViewConfig {
    Simple {
        font_scale: u32,
        fg_color: Color,
        bg_color: Color,
    },
    Markdown {
        normal: Font,
        emphasis: Font,
        strong: Font,
    },
}

pub struct Document<'b> {
    path: Option<String>,
    text_buffer: TextBuffer<'b>,
    caret: usize,
    edit_mode: EditMode,
    config: TextEditorConfig,
    current_view: ViewConfig,
    scroll_offset: u32,
}

impl<'b> Document<'b> {
    pub fn new(
        path: Option<String>,
        text_buffer: TextBuffer<'b>,
        config: TextEditorConfig,
    ) -> Document<'b> {
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
    pub fn view_config(&self) -> &ViewConfig {
        &self.current_view
    }
    pub fn scroll_offset(&self) -> u32 {
        self.scroll_offset
    }
    pub fn caret(&self) -> usize {
        self.caret
    }
    fn next_line(&self, pos: usize) -> Option<usize> {
        let mut index = 0;
        if self.text_buffer.get_char(pos).is_some_and(|c| c == '\n') {
            if self.text_buffer.get_char(pos + 1).is_some() {
                return Some(pos + 1);
            }
            return None;
        }
        while let Some(c) = self.text_buffer.get_char(pos + index) {
            if c == '\n' && index > 0 {
                break;
            }
            index += 1;
        }
        if self.text_buffer.get_char(pos + index + 1).is_some() {
            return Some(pos + index + 1);
        } else if self.text_buffer.get_char(pos + index).is_some() {
            return Some(pos + index);
        }
        None
    }
    fn prev_line(&self, pos: usize) -> Option<usize> {
        let mut index = 0;
        /*if self.text_buffer.get_char(pos).is_some_and(|c| c == '\n') {
            if pos.checked_sub(1).is_some() && self.text_buffer.get_char(pos - 1).is_some() {
                return Some(pos - 1);
            }
            return None;
        }*/
        while let Some(c) = self.text_buffer.get_char(pos - index) {
            if c == '\n' && index > 0 {
                break;
            }
            index += 1;
            if pos < index {
                break;
            }
        }
        if pos < index {
            return Some(0);
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
        let next_next_line = self.next_line(next_line);
        if self
            .text_buffer
            .get_char(next_line)
            .is_some_and(|c| c == '\n')
        {
            self.caret = next_line;
            return;
        } else if next_next_line.is_some_and(|n| n - next_line <= origin_len) {
            self.caret = next_next_line.unwrap() - 1;
            return;
        }
        self.caret = next_line + origin_len;
        #[cfg(feature = "with_runtime")]
        warn!(
            "prev line {} origin_len {} nex_line {}",
            prev_line, origin_len, next_line
        );
    }

    fn move_cursor_up(&mut self) {
        let text = self.text_buffer.to_string();
        let prev_line = self.prev_line(self.caret).unwrap_or(self.caret);
        let origin_len = self.caret - prev_line;
        let prev_prev_line = self.prev_line(prev_line - 2).unwrap_or(self.caret);
        self.caret = prev_prev_line + origin_len;
    }

    fn update_insert(&mut self, k: DecodedKey) {
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
            DecodedKey::RawKey(k) => {
                #[cfg(feature = "with_runtime")]
                warn!("TextEditor can't process input: {:?}", k);
            }
        }
    }

    fn update_normal(&mut self, k: DecodedKey) {
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
    }

    fn scroll(&mut self, msg: ViewMessage) {
        match msg {
            ViewMessage::ScrollDown(v) => self.scroll_offset += v,
            ViewMessage::ScrollUp(v) => {
                self.scroll_offset = self.scroll_offset.checked_sub(v).unwrap_or(0)
            }
        }
    }

    pub fn update(&mut self, m: Message) {
        match m {
            Message::ViewMessage(msg) => self.scroll(msg),
            Message::DecodedKey(k) => match self.edit_mode {
                EditMode::Insert => self.update_insert(k),
                EditMode::Normal => self.update_normal(k),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use graphic::color::{Color, WHITE};

    use crate::apps::text_editor::font::Font;

    use super::*;

    fn generate_dummy_config() -> TextEditorConfig {
        let bg_color = Color {
            red: 20,
            green: 20,
            blue: 20,
            alpha: 255,
        };
        let font = Font {
            scale: 1,
            fg_color: WHITE,
            bg_color: bg_color,
            char_height: 20,
            char_width: 20,
        };
        TextEditorConfig {
            width: 200,
            height: 200,
            background_color: bg_color,
            markdown_view: View::Markdown {
                normal: font,
                emphasis: font,
                strong: font,
            },
            simple_view: View::Simple {
                font_scale: 1,
                fg_color: WHITE,
                bg_color: bg_color,
            },
        }
    }
    #[test]
    fn move_cursor_down_0() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 0;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 4);
    }

    #[test]
    fn move_cursor_down_1() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 1;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 5);
    }
    #[test]
    fn move_cursor_down_2() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 3;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 7);
    }
    #[test]
    fn move_cursor_down_3() {
        let text = "Das\nein\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 3;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 7);
    }
    #[test]
    fn move_cursor_down_4() {
        let text = "Das\nein\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 5;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 9);
    }
    #[test]
    fn move_cursor_down_with_space_0() {
        let text = "Das\n";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 0;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 3);
    }
    #[test]
    fn move_cursor_down_with_space_1() {
        let text = "Das\n\nein";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 0;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 4);
    }
    #[test]
    fn move_cursor_down_with_space_2() {
        let text = "Das\n\nein";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 1;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 4);
    }
    #[test]
    fn move_cursor_down_with_space_3() {
        let text = "Das\n\nein";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 3;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 4);
    }
    #[test]
    fn move_cursor_down_with_space_4() {
        let text = "Das\n\nein";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 4;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 5);
    }
    #[test]
    fn move_cursor_down_shoreter_0() {
        let text = "Hallo\nein";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 4;
        doc.move_cursor_down();
        // Nicht hundert Prozent richtig aber gut genug
        assert_eq!(doc.caret(), 10);
    }
    #[test]
    fn move_cursor_down_shoreter_1() {
        let text = "Hallo\nein\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret = 4;
        doc.move_cursor_down();
        assert_eq!(doc.caret(), 9);
    }
}
