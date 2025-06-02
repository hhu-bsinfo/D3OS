use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::{u32::MAX, usize};
use drawer::vertex::Vertex;
use graphic::{
    bitmap::{self, Bitmap},
    color::{Color, WHITE, YELLOW},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use logger::warn;
use pulldown_cmark::{Event, HeadingLevel, OffsetIter, Parser, TextMergeStream};
use text_buffer::TextBuffer;

use super::{meassages::ViewMessage, model::Document};
//Julius Drodofsky

#[derive(Debug, Clone, Copy)]
pub struct Font {
    pub scale: u32,
    pub fg_color: Color,
    pub bg_color: Color,
    pub char_width: u32,
    pub char_height: u32,
}

impl Font {
    pub fn add_scale(&self, add: u32) -> Font {
        let mut ret = *self;
        ret.scale += add;
        ret
    }
}

#[derive(Debug, Clone, Copy)]
pub enum View {
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
    ) -> Option<ViewMessage> {
        let mut x = 0;
        let mut y = 0;
        let mut i: usize = document.scroll_offset() as usize;
        let mut found_caret = false;
        let mut new_lines = Vec::<u32>::new();
        let mut caret_pos: u32 = 0;
        buffer.clear(bg_color);
        while let Some(c) = document.text_buffer().get_char(i) {
            if y + DEFAULT_CHAR_HEIGHT * font_scale >= buffer.height {
                break;
            }
            if i == document.caret() {
                buffer.draw_line(x, y, x, y + DEFAULT_CHAR_HEIGHT * font_scale, YELLOW);
                caret_pos = y;
                found_caret = true;
                warn!("caret: {}", document.caret())
            }
            if c == '\n' {
                y += DEFAULT_CHAR_HEIGHT * font_scale;
                x = 0;
                i += 1;
                new_lines.push(i as u32);
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
        if !found_caret {
            if document.caret() >= document.scroll_offset() as usize {
                let ind = new_lines.len() / 3;
                let scroll = new_lines[ind] - document.scroll_offset();
                return Some(ViewMessage::ScrollDown(scroll));
            } else {
                return Some(ViewMessage::ScrollUp(document.scroll_offset()));
            }
        } else if caret_pos > buffer.height / 2 + buffer.height / 3 {
            let scroll = *match new_lines.first() {
                Some(v) => v,
                None => return None,
            } - document.scroll_offset();

            return Some(ViewMessage::ScrollDown(scroll));
        }
        None
    }
    fn render_markdown(
        &self,
        document: &Document,
        buffer: &mut Bitmap,
        normal: Font,
        emphasis: Font,
        strong: Font,
    ) -> Option<ViewMessage> {
        buffer.clear(normal.bg_color);
        let raw_text = document.text_buffer().to_string();
        let iterator = Parser::new(&raw_text).into_offset_iter();
        let mut position = Vertex::zero();
        let mut font = Vec::<Font>::new();
        let mut heading = false;
        font.push(normal);
        for (event, range) in iterator {
            match event {
                Event::Text(text) => {
                    let rel_caret = document.caret().checked_sub(range.start);
                    if heading {
                        position = self.render_string(
                            &String::from("\n"),
                            buffer,
                            *font.last().unwrap_or(&normal),
                            position,
                            rel_caret,
                        );
                    }
                    position = self.render_string(
                        &text.to_string(),
                        buffer,
                        *font.last().unwrap_or(&normal),
                        position,
                        rel_caret,
                    );
                    if heading {
                        position = self.render_string(
                            &String::from("\n"),
                            buffer,
                            *font.last().unwrap_or(&normal),
                            position,
                            rel_caret,
                        );
                    }
                }
                Event::HardBreak | Event::SoftBreak => {
                    let rel_caret = document.caret().checked_sub(range.start);
                    position = self.render_string(
                        &String::from("\n"),
                        buffer,
                        *font.last().unwrap_or(&normal),
                        position,
                        rel_caret,
                    );
                }
                Event::Start(t) => match t {
                    pulldown_cmark::Tag::Emphasis => font.push(emphasis),
                    pulldown_cmark::Tag::Strong => font.push(strong),
                    pulldown_cmark::Tag::Heading {
                        level,
                        id,
                        classes,
                        attrs,
                    } => {
                        heading = true;
                        match level {
                            HeadingLevel::H1 => font.push(strong.add_scale(1)),
                            HeadingLevel::H2 => font.push(emphasis.add_scale(1)),
                            HeadingLevel::H3 => font.push(normal.add_scale(1)),
                            HeadingLevel::H4 => font.push(strong),
                            HeadingLevel::H5 => font.push(emphasis),
                            HeadingLevel::H6 => font.push(normal),
                        }
                    }
                    _ => (),
                },
                Event::Rule => {
                    let rel_caret = document.caret().checked_sub(range.start);
                    position = self.render_string(
                        &String::from("\n"),
                        buffer,
                        *font.last().unwrap_or(&normal),
                        position,
                        rel_caret,
                    );
                    buffer.draw_line(
                        ((buffer.width as f32 * 0.1) as u32),
                        position.y + (normal.char_height * normal.scale / 2),
                        ((buffer.width as f32) * 0.9) as u32,
                        position.y + (normal.char_height * normal.scale / 2),
                        WHITE,
                    );
                    position = self.render_string(
                        &String::from("\n"),
                        buffer,
                        *font.last().unwrap_or(&normal),
                        position,
                        rel_caret,
                    );
                }
                Event::End(t) => match t {
                    pulldown_cmark::TagEnd::Emphasis => {
                        font.pop();
                    }
                    pulldown_cmark::TagEnd::Strong => {
                        font.pop();
                    }
                    pulldown_cmark::TagEnd::Heading(l) => {
                        heading = false;
                        match l {
                            _ => {
                                font.pop();
                            }
                        }
                    }
                    _ => (),
                },
                _ => {}
            }
        }
        None
    }
    pub fn render(&self, document: &Document, buffer: &mut Bitmap) -> Option<ViewMessage> {
        match self {
            View::Simple {
                font_scale,
                fg_color,
                bg_color,
            } => self.render_simple(document, buffer, *font_scale, *fg_color, *bg_color),
            View::Markdown {
                normal,
                emphasis,
                strong,
            } => self.render_markdown(document, buffer, *normal, *emphasis, *strong),
        }
    }
}
