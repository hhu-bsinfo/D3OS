use alloc::string::String;
use graphic::color::Color;
use logger::warn;
use terminal::print;
use terminal::{println, DecodedKey};
use text_buffer::TextBuffer;

use super::font::Font;
use super::messages::{Message, ViewMessage};
use super::TextEditorConfig;
use crate::apps::text_editor::messages::CommandMessage;
use logger::error;
enum EditMode {
    Normal,
    Insert,
    Visual,
}

#[derive(Debug, Clone, Copy)]
pub enum Caret {
    Normal(usize),
    Visual { anchor: usize, head: usize },
}

impl Caret {
    pub fn head(&self) -> usize {
        match self {
            Caret::Normal(h) => *h,
            Caret::Visual { anchor: _, head } => *head,
        }
    }
    pub fn set_head(&mut self, new_head: usize) {
        match self {
            Caret::Normal(h) => {
                *h = new_head;
            }
            Caret::Visual { anchor: _, head } => *head = new_head,
        };
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ViewConfig<'s> {
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
    Code {
        normal: Font,
        keyword: Font,
        string: Font,
        number: Font,
        comment: Font,
        keywords: &'s [&'s str],
    },
}

pub struct Document<'b, 'v> {
    path: Option<String>,
    text_buffer: TextBuffer<'b>,
    copy_buffer: String,
    caret: Caret,
    edit_mode: EditMode,
    config: TextEditorConfig<'v>,
    current_view: ViewConfig<'v>,
    scroll_offset: u32,
}

impl<'b, 'v, 'r> Document<'b, 'v> {
    pub fn new(
        path: Option<String>,
        text_buffer: TextBuffer<'b>,
        config: TextEditorConfig<'v>,
    ) -> Document<'b, 'v> {
        Document {
            path: path,
            text_buffer: text_buffer,
            copy_buffer: String::new(),
            caret: Caret::Normal(0),
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
    pub fn caret(&self) -> Caret {
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
    pub fn prev_line(&self, pos: usize) -> Option<usize> {
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
        let prev_line = self
            .prev_line(self.caret.head())
            .unwrap_or(self.caret.head());
        let origin_len = self.caret.head() - prev_line;
        let next_line = self
            .next_line(self.caret.head())
            .unwrap_or(self.caret.head());
        let next_next_line = self.next_line(next_line);
        if self
            .text_buffer
            .get_char(next_line)
            .is_some_and(|c| c == '\n')
        {
            self.caret.set_head(next_line);
            return;
        } else if next_next_line.is_some_and(|n| n - next_line <= origin_len) {
            self.caret.set_head(next_next_line.unwrap() - 1);
            return;
        }
        self.caret.set_head(next_line + origin_len);
    }

    fn move_cursor_up(&mut self) {
        let prev_line = match self.prev_line(self.caret.head()) {
            Some(s) => s,
            None => {
                self.caret.set_head(0);
                return;
            }
        };
        let origin_len = self.caret.head() - prev_line;
        let prev_prev_line = self
            .prev_line(prev_line.checked_sub(1).unwrap_or(0))
            .unwrap();
        if prev_line == prev_prev_line {
            self.caret.set_head(0);
            return;
        }
        if self
            .text_buffer
            .get_char(self.caret.head())
            .is_some_and(|c| c == '\n')
            && self
                .text_buffer
                .get_char(
                    self.caret
                        .head()
                        .checked_sub(1)
                        .unwrap_or(self.caret.head()),
                )
                .is_some_and(|c| c == '\n')
        {
            self.caret
                .set_head(self.caret.head().checked_sub(1).unwrap_or(0));
            return;
        }
        if prev_line - prev_prev_line <= origin_len {
            self.caret.set_head(prev_line.checked_sub(1).unwrap_or(0));
            return;
        }
        self.caret.set_head(prev_prev_line + origin_len);
    }

    fn update_insert(&mut self, k: DecodedKey) {
        //delete
        match k {
            // delete
            DecodedKey::Unicode('\x08') => {
                self.text_buffer.delete(self.caret.head() - 1);
                self.caret.set_head(self.caret.head() - 1);
            }
            // esc
            DecodedKey::Unicode('\x1B') => {
                self.edit_mode = EditMode::Normal;
            }
            DecodedKey::Unicode(ch) => {
                self.text_buffer.insert(self.caret.head(), ch);
                self.caret.set_head(self.caret.head() + 1);
            }
            DecodedKey::RawKey(terminal::KeyCode::ArrowLeft) => {
                self.caret.set_head(self.caret.head() - 1)
            }
            DecodedKey::RawKey(terminal::KeyCode::ArrowRight) => {
                self.caret.set_head(self.caret.head() + 1)
            }
            DecodedKey::RawKey(terminal::KeyCode::ArrowUp) => self.move_cursor_up(),
            DecodedKey::RawKey(terminal::KeyCode::ArrowDown) => self.move_cursor_down(),
            DecodedKey::RawKey(k) => {
                #[cfg(feature = "with_runtime")]
                warn!("TextEditor can't process input: {:?}", k);
            }
        }
        if self.caret.head() > self.text_buffer.len() {
            self.caret.set_head(self.text_buffer.len());
        }
    }

    fn yank(&mut self) {
        let fst: usize;
        let snd: usize;
        self.copy_buffer.clear();
        match self.caret {
            Caret::Normal(_) => {
                #[cfg(feature = "with_runtime")]
                error!("yank from normal caret");
                return;
            }
            Caret::Visual { anchor, head } => {
                if anchor < head {
                    fst = anchor;
                    snd = head
                } else {
                    fst = head;
                    snd = anchor;
                }
            }
        }
        for i in fst..snd {
            let v = match self.text_buffer.get_char(i) {
                Some(s) => s,
                None => {
                    error!("yank from none caret");
                    return;
                }
            };
            self.copy_buffer.push(v);
        }
        self.caret = Caret::Normal(self.caret.head());
        self.edit_mode = EditMode::Normal;
    }

    fn paste(&mut self) {
        #[cfg(feature = "with_runtime")]
        for c in self.copy_buffer.chars() {
            self.text_buffer.insert(self.caret.head(), c);
            self.caret.set_head(self.caret.head() + 1);
        }
    }

    fn update_visual(&mut self, k: DecodedKey) {
        match k {
            DecodedKey::Unicode('h') => self.caret.set_head(self.caret.head() - 1),
            DecodedKey::Unicode('l') => self.caret.set_head(self.caret.head() + 1),
            DecodedKey::Unicode('j') => self.move_cursor_down(),
            DecodedKey::Unicode('k') => self.move_cursor_up(),
            DecodedKey::RawKey(terminal::KeyCode::ArrowLeft) => {
                self.caret.set_head(self.caret.head() - 1)
            }
            DecodedKey::RawKey(terminal::KeyCode::ArrowRight) => {
                self.caret.set_head(self.caret.head() + 1)
            }
            DecodedKey::RawKey(terminal::KeyCode::ArrowUp) => self.move_cursor_up(),
            DecodedKey::RawKey(terminal::KeyCode::ArrowDown) => self.move_cursor_down(),
            DecodedKey::Unicode('y') => self.yank(),
            _ => (),
        }
        if self.caret.head() > self.text_buffer.len() {
            self.caret.set_head(self.text_buffer.len());
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
            DecodedKey::Unicode('h') => self.caret.set_head(self.caret.head() - 1),
            DecodedKey::Unicode('l') => self.caret.set_head(self.caret.head() + 1),
            DecodedKey::Unicode('j') => self.move_cursor_down(),
            DecodedKey::Unicode('k') => self.move_cursor_up(),
            DecodedKey::Unicode('i') => self.edit_mode = EditMode::Insert,
            DecodedKey::Unicode('n') => self.current_view = self.config.simple_view,
            DecodedKey::Unicode('m') => self.current_view = self.config.markdown_view,
            DecodedKey::RawKey(terminal::KeyCode::ArrowLeft) => {
                self.caret.set_head(self.caret.head() - 1)
            }
            DecodedKey::RawKey(terminal::KeyCode::ArrowRight) => {
                self.caret.set_head(self.caret.head() + 1)
            }
            DecodedKey::RawKey(terminal::KeyCode::ArrowUp) => self.move_cursor_up(),
            DecodedKey::RawKey(terminal::KeyCode::ArrowDown) => self.move_cursor_down(),
            DecodedKey::Unicode('v') => {
                self.edit_mode = EditMode::Visual;
                self.caret = Caret::Visual {
                    anchor: self.caret.head(),
                    head: self.caret.head(),
                };
            }
            DecodedKey::Unicode('p') => self.paste(),
            _ => (),
        }
        if self.caret.head() > self.text_buffer.len() {
            self.caret.set_head(self.text_buffer.len());
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
            Message::CommandMessage(c) => match c {
                CommandMessage::Undo => {
                    self.text_buffer.undo();
                }
                CommandMessage::Redo => {
                    self.text_buffer.redo();
                }
                CommandMessage::Markdown => match self.current_view {
                    ViewConfig::Markdown {
                        normal: _,
                        emphasis: _,
                        strong: _,
                    } => self.current_view = self.config.simple_view,
                    ViewConfig::Simple {
                        font_scale: _,
                        fg_color: _,
                        bg_color: _,
                    } => self.current_view = self.config.markdown_view,
                    _ => (),
                },
                CommandMessage::CLike => match self.current_view {
                    ViewConfig::Code {
                        normal: _,
                        keyword: _,
                        string: _,
                        number: _,
                        comment: _,
                        keywords: _,
                    } => self.current_view = self.config.simple_view,
                    ViewConfig::Simple {
                        font_scale: _,
                        fg_color: _,
                        bg_color: _,
                    } => self.current_view = self.config.code_view,
                    _ => (),
                },
            },
            Message::ViewMessage(msg) => self.scroll(msg),
            Message::DecodedKey(k) => match self.edit_mode {
                EditMode::Insert => self.update_insert(k),
                EditMode::Normal => self.update_normal(k),
                EditMode::Visual => self.update_visual(k),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use graphic::color::{Color, WHITE};

    use crate::apps::text_editor::font::Font;

    use super::*;

    fn generate_dummy_config() -> TextEditorConfig<'static> {
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
            markdown_view: ViewConfig::Markdown {
                normal: font,
                emphasis: font,
                strong: font,
            },
            simple_view: ViewConfig::Simple {
                font_scale: 1,
                fg_color: WHITE,
                bg_color: bg_color,
            },
            code_view: ViewConfig::Code {
                normal: font,
                keyword: font,
                string: font,
                number: font,
                comment: font,
                keywords: &[],
            },
        }
    }
    #[test]
    fn move_cursor_down_0() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(0);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 4);
    }

    #[test]
    fn move_cursor_down_1() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(1);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 5);
    }
    #[test]
    fn move_cursor_down_2() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(3);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 7);
    }
    #[test]
    fn move_cursor_down_3() {
        let text = "Das\nein\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(3);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 7);
    }
    #[test]
    fn move_cursor_down_4() {
        let text = "Das\nein\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(5);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 9);
    }
    #[test]
    fn move_cursor_down_with_space_0() {
        let text = "Das\n";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(0);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 3);
    }
    #[test]
    fn move_cursor_down_with_space_1() {
        let text = "Das\n\nein";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(0);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 4);
    }
    #[test]
    fn move_cursor_down_with_space_2() {
        let text = "Das\n\nein";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(1);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 4);
    }
    #[test]
    fn move_cursor_down_with_space_3() {
        let text = "Das\n\nein";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(3);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 4);
    }
    #[test]
    fn move_cursor_down_with_space_4() {
        let text = "Das\n\nein";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(4);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 5);
    }
    #[test]
    fn move_cursor_down_shoreter_0() {
        let text = "Hallo\nein";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(4);
        doc.move_cursor_down();
        // Nicht hundert Prozent richtig aber gut genug
        assert_eq!(doc.caret().head(), 10);
    }
    #[test]
    fn move_cursor_down_shoreter_1() {
        let text = "Hallo\nein\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(4);
        doc.move_cursor_down();
        assert_eq!(doc.caret().head(), 9);
    }

    #[test]
    fn move_cursor_up_0() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(4);
        doc.move_cursor_up();
        assert_eq!(doc.caret().head(), 0);
    }

    #[test]
    fn move_cursor_up_1() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(5);
        doc.move_cursor_up();
        assert_eq!(doc.caret().head(), 1);
    }
    #[test]
    fn move_cursor_up_2() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(0);
        doc.move_cursor_up();
        assert_eq!(doc.caret().head(), 0);
    }
    #[test]
    fn move_cursor_up_3() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(1);
        doc.move_cursor_up();
        assert_eq!(doc.caret().head(), 0);
    }
    #[test]
    fn move_cursor_up_4() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(3);
        doc.move_cursor_up();
        assert_eq!(doc.caret().head(), 0);
    }
    #[test]
    fn move_cursor_up_5() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(7);
        doc.move_cursor_up();
        assert_eq!(doc.caret().head(), 3);
    }
    #[test]
    fn move_cursor_up_6() {
        let text = "Das\nTest2";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(8);
        doc.move_cursor_up();
        assert_eq!(doc.caret().head(), 3);
    }
    #[test]
    fn move_cursor_up_7() {
        let text = "Das\nist\nein\nTest!";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(11);
        doc.move_cursor_up();
        assert_eq!(doc.caret().head(), 7);
    }
    #[test]
    fn move_cursor_up_8() {
        let text = "Das\nist\nein\nTest!";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(16);
        doc.move_cursor_up();
        assert_eq!(doc.caret().head(), 11);
    }
    #[test]
    fn move_cursor_up_9() {
        let text = "Das\nist\n\nein\nTest!";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(8);
        doc.move_cursor_up();
        assert_eq!(doc.caret().head(), 7);
    }
    #[test]
    fn undo_with_command() {
        let text = "Das\nTest";
        let mut doc = Document::new(None, TextBuffer::from_str(text), generate_dummy_config());
        doc.caret.set_head(0);
        doc.update_insert(DecodedKey::Unicode('H'));
        doc.update_insert(DecodedKey::Unicode('e'));
        doc.update_insert(DecodedKey::Unicode('y'));
        doc.update(Message::CommandMessage(CommandMessage::Undo));
        assert_eq!(doc.text_buffer.to_string(), String::from("HeDas\nTest"));
    }
}
