use core::usize;

use alloc::string::String;
use drawer::vertex::Vertex;
use graphic::{
    bitmap::{self, Bitmap},
    color::{Color, YELLOW},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use text_buffer::TextBuffer;

use super::model::Document;
//Julius Drodofsky

pub struct Font {
    pub scale: u32,
    pub fg_color: Color,
    pub bg_color: Color,
    pub char_width: u32,
    pub char_height: u32,
}

pub enum View {
    Simple {
        font_scale: u32,
        fg_color: Color,
        bg_color: Color,
    },
    Markdown {
        normal: Font,
        bold: Font,
        italic: Font,
    },
}

impl View {
    fn render_string(
        &self,
        text: &String,
        buffer: &mut Bitmap,
        font: Font,
        position: Vertex,
        rel_caret: Option<usize>,
    ) -> Vertex {
        let mut x = position.x;
        let mut y = position.y;
        let mut i = 0;
        while let Some(c) = text.chars().nth(i) {
            if i == rel_caret.unwrap_or(usize::MAX) {
                buffer.draw_line(x, y, x, y + font.char_height * font.scale, YELLOW);
            }
            if c == '\n' {
                y += font.char_height * font.scale;
                x = 0;
                i += 1;
                continue;
            }
            if buffer.width - x + 1 < font.char_width * font.scale {
                x = 0;
                y += font.char_height * font.scale;
            }
            x += buffer.draw_char_scaled(
                x + 1,
                y,
                font.scale,
                font.scale,
                font.fg_color,
                font.bg_color,
                c,
            ) * font.scale
                + 1;
            i += 1;
        }
        if i == rel_caret.unwrap_or(usize::MAX) {
            buffer.draw_line(x, y, x, y + font.char_height * font.scale, YELLOW);
        }
        return Vertex { x: x, y: y };
    }
    fn render_simple(
        &self,
        document: &Document,
        buffer: &mut Bitmap,
        font_scale: u32,
        fg_color: Color,
        bg_color: Color,
    ) {
        let mut x = 0;
        let mut y = 0;
        let mut i: usize = 0;
        buffer.clear(bg_color);
        while let Some(c) = document.text_buffer().get_char(i) {
            if i == document.caret() {
                buffer.draw_line(x, y, x, y + DEFAULT_CHAR_HEIGHT * font_scale, YELLOW);
            }
            if c == '\n' {
                y += DEFAULT_CHAR_HEIGHT * font_scale;
                x = 0;
                i += 1;
                continue;
            }
            if buffer.width - x + 1 < DEFAULT_CHAR_WIDTH * font_scale {
                x = 0;
                y += DEFAULT_CHAR_HEIGHT * font_scale;
            }
            x += buffer.draw_char_scaled(x + 1, y, font_scale, font_scale, fg_color, bg_color, c)
                * font_scale
                + 1;
            i += 1;
        }
        if i == document.caret() {
            buffer.draw_line(x, y, x, y + DEFAULT_CHAR_HEIGHT * font_scale, YELLOW);
        }
    }
    pub fn render(&self, document: &Document, buffer: &mut Bitmap) {
        match self {
            View::Simple {
                font_scale,
                fg_color,
                bg_color,
            } => self.render_simple(document, buffer, *font_scale, *fg_color, *bg_color),
            _ => (),
        }
    }
}
