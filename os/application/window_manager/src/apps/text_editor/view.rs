use crate::apps::text_editor::model::Caret;
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::ops::Range;
use core::usize;
use drawer::vertex::Vertex;
use graphic::{
    bitmap::Bitmap,
    color::{Color, WHITE, YELLOW},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use logger::warn;
use pulldown_cmark::{Event, HeadingLevel, Parser};
use syntax::clike::{parse_clike, Token};
use syntax::located::Located;

use super::{
    font::Font,
    messages::ViewMessage,
    model::{Document, ViewConfig},
};
//Julius Drodofsky

trait VecScrollDown<T> {
    fn scroll_down(&self) -> Option<&T>;
}
// The value (5) specify to which line to scroll a higher value means more redraws
// it should not be lower than 1
impl<T> VecScrollDown<T> for Vec<T> {
    fn scroll_down(&self) -> Option<&T> {
        if self.is_empty() {
            None
        } else {
            self.get(self.len() / 5)
        }
    }
}

pub struct View;

impl View {
    fn render_string(
        text: &String,
        buffer: &mut Bitmap,
        font: Font,
        position: Vertex,
        rel_caret: Option<usize>,
    ) -> (Vertex, Vec<u32>) {
        let mut x = position.x;
        let mut y = position.y;
        let mut i = 0;
        let mut new_lines = Vec::<u32>::new();
        while let Some(c) = text.chars().nth(i) {
            if i == rel_caret.unwrap_or(usize::MAX) {
                buffer.draw_line(x, y, x, y + font.char_height * font.scale, YELLOW);
            }
            if c == '\n' {
                y += font.char_height * font.scale;
                x = 0;
                i += 1;
                new_lines.push(i as u32);
                continue;
            }
            if buffer.width - x + 1 < font.char_width * font.scale {
                x = 0;
                y += font.char_height * font.scale;
                new_lines.push(i as u32);
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
        return (Vertex { x: x, y: y }, new_lines);
    }
    fn render_simple(
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
        let visual: Option<(usize, usize)> = match document.caret() {
            Caret::Visual { anchor, head } => Some((anchor, head)),
            Caret::Normal(_) => None,
        };
        while let Some(c) = document.text_buffer().get_char(i) {
            if y + DEFAULT_CHAR_HEIGHT * font_scale >= buffer.height {
                break;
            }
            if i == document.caret().head() {
                buffer.draw_line(x, y, x, y + DEFAULT_CHAR_HEIGHT * font_scale, YELLOW);
                caret_pos = y;
                found_caret = true;
                warn!("caret: {}", document.caret().head())
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
            if visual.is_some_and(|(x, y)| i >= x && i < y) {
                buffer.draw_line(x, y, x, y + DEFAULT_CHAR_HEIGHT * font_scale - 1, fg_color);
                x += buffer.draw_char_scaled(
                    x + 1,
                    y,
                    font_scale,
                    font_scale,
                    bg_color,
                    fg_color,
                    c,
                ) * font_scale
                    + 1;
                i += 1;
                if i == document.caret().head() {
                    buffer.draw_line(x, y, x, y + DEFAULT_CHAR_HEIGHT * font_scale, YELLOW);
                    caret_pos = y;
                    found_caret = true;
                }
                continue;
            }
            x += buffer.draw_char_scaled(x + 1, y, font_scale, font_scale, fg_color, bg_color, c)
                * font_scale
                + 1;
            i += 1;
        }
        if i == document.caret().head() {
            buffer.draw_line(x, y, x, y + DEFAULT_CHAR_HEIGHT * font_scale, YELLOW);
        }
        // Calculate new scrolling (cursor not in frame)
        if !found_caret {
            if document.caret().head() >= document.scroll_offset() as usize {
                let ind = new_lines.len() / 3;
                let scroll = new_lines[ind] - document.scroll_offset();
                return Some(ViewMessage::ScrollDown(scroll));
            } else {
                return Some(ViewMessage::ScrollUp(
                    document.scroll_offset()
                        - document
                            .prev_line(document.caret().head())
                            .unwrap_or(document.scroll_offset() as usize)
                            as u32,
                ));
            }
            // scroll down (cursor in frame)
        } else if caret_pos > buffer.height / 2 + buffer.height / 3 {
            let scroll = *match new_lines.first() {
                Some(v) => v,
                None => return None,
            } - document.scroll_offset();

            return Some(ViewMessage::ScrollDown(scroll));
        } else if caret_pos < buffer.height / 3 && document.scroll_offset() != 0 {
            let scroll = match document.prev_line(
                document
                    .scroll_offset()
                    .checked_sub(1)
                    .unwrap_or(document.scroll_offset()) as usize,
            ) {
                Some(v) => v,
                None => return None,
            };
            if document.scroll_offset() - scroll as u32 <= 0 {
                return None;
            }
            return Some(ViewMessage::ScrollUp(
                document.scroll_offset() - scroll as u32,
            ));
        }
        None
    }
    fn render_markdown(
        document: &Document,
        buffer: &mut Bitmap,
        normal: Font,
        emphasis: Font,
        strong: Font,
    ) -> Option<ViewMessage> {
        buffer.clear(normal.bg_color);
        let raw_text: String = document.text_buffer().to_string();

        let iterator = Parser::new(&raw_text).into_offset_iter();
        let mut position = Vertex::zero();
        let mut font = Vec::<Font>::new();
        let mut list_indentation: usize = 0;
        let mut first_in_list_item = false;
        let mut ordererd_start = false;
        let mut ordered: Option<u64> = None;
        let mut heading = false;
        let mut last_index: Range<usize> = 0..0;
        let mut tmp_line_start: Vec<u32> = Vec::new();
        let mut line_start: Vec<u32> = Vec::new();
        font.push(normal);
        for (event, range) in iterator {
            if range.end < document.scroll_offset() as usize {
                continue;
            }
            let rel_caret = document.caret().head().checked_sub(range.start);
            if position.y > buffer.height {
                break;
            }
            last_index = range.clone();
            match event {
                Event::Text(text) => {
                    if heading {
                        (position, tmp_line_start) = View::render_string(
                            &String::from("\n"),
                            buffer,
                            *font.last().unwrap_or(&normal),
                            position,
                            rel_caret,
                        );
                    }
                    let mut s = String::new();
                    if list_indentation > 0 {
                        if first_in_list_item {
                            if ordered.is_some() {
                                s = ordered.unwrap().to_string();
                                s.push('.');
                            }

                            (position, tmp_line_start) = View::render_string(
                                &format!("{}{} ", " ".repeat(list_indentation), s),
                                buffer,
                                *font.last().unwrap_or(&normal),
                                position,
                                rel_caret,
                            );
                            if ordered.is_none() {
                                buffer.draw_circle_bresenham(
                                    (
                                        position.x,
                                        position.y + (normal.char_height * normal.scale / 2 + 1),
                                    ),
                                    2 * normal.scale,
                                    normal.fg_color,
                                );
                                position.x += normal.char_width * normal.scale;
                            }
                            first_in_list_item = false;
                        }
                    }
                    (position, tmp_line_start) = View::render_string(
                        &text.to_string(),
                        buffer,
                        *font.last().unwrap_or(&normal),
                        position,
                        rel_caret,
                    );
                    if heading {
                        (position, tmp_line_start) = View::render_string(
                            &String::from("\n"),
                            buffer,
                            *font.last().unwrap_or(&normal),
                            position,
                            rel_caret,
                        );
                    }
                }
                Event::HardBreak | Event::SoftBreak => {
                    (position, tmp_line_start) = View::render_string(
                        &String::from("\n"),
                        buffer,
                        *font.last().unwrap_or(&normal),
                        position,
                        rel_caret,
                    );
                }
                Event::Start(t) => match t {
                    pulldown_cmark::Tag::Paragraph => {
                        (position, tmp_line_start) = View::render_string(
                            &String::from("\n"),
                            buffer,
                            *font.last().unwrap_or(&normal),
                            position,
                            rel_caret,
                        );
                    }
                    pulldown_cmark::Tag::Item => {
                        first_in_list_item = true;
                        if ordererd_start {
                            ordererd_start = false;
                            continue;
                        }
                        match ordered {
                            Some(s) => ordered = Some(s + 1),
                            None => (),
                        }
                    }
                    pulldown_cmark::Tag::List(s) => {
                        list_indentation += 2;
                        first_in_list_item = true;
                        ordererd_start = true;
                        ordered = s;
                        (position, tmp_line_start) = View::render_string(
                            &String::from("\n"),
                            buffer,
                            *font.last().unwrap_or(&normal),
                            position,
                            rel_caret,
                        );
                    }
                    pulldown_cmark::Tag::Emphasis => font.push(emphasis),
                    pulldown_cmark::Tag::Strong => font.push(strong),
                    pulldown_cmark::Tag::Heading {
                        level,
                        id: _,
                        classes: _,
                        attrs: _,
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
                    (position, tmp_line_start) = View::render_string(
                        &String::from("\n"),
                        buffer,
                        *font.last().unwrap_or(&normal),
                        position,
                        rel_caret,
                    );
                    buffer.draw_line(
                        (buffer.width as f32 * 0.1) as u32,
                        position.y + (normal.char_height * normal.scale / 2),
                        ((buffer.width as f32) * 0.9) as u32,
                        position.y + (normal.char_height * normal.scale / 2),
                        WHITE,
                    );
                    (position, tmp_line_start) = View::render_string(
                        &String::from("\n"),
                        buffer,
                        *font.last().unwrap_or(&normal),
                        position,
                        rel_caret,
                    );
                }
                Event::End(t) => match t {
                    pulldown_cmark::TagEnd::Paragraph => {
                        (position, tmp_line_start) = View::render_string(
                            &String::from("\n"),
                            buffer,
                            *font.last().unwrap_or(&normal),
                            position,
                            rel_caret,
                        );
                    }
                    pulldown_cmark::TagEnd::Item => {
                        (position, tmp_line_start) = View::render_string(
                            &String::from("\n"),
                            buffer,
                            *font.last().unwrap_or(&normal),
                            position,
                            rel_caret,
                        );
                    }
                    pulldown_cmark::TagEnd::List(_) => {
                        list_indentation = list_indentation.checked_sub(2).unwrap_or(0);
                    }
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
            for e in tmp_line_start.iter() {
                line_start.push(e + range.start as u32);
            }
        }
        // scroll up if curosor bevore visible document
        if document.scroll_offset() > document.caret().head() as u32 {
            return Some(ViewMessage::ScrollUp(
                document
                    .scroll_offset()
                    .checked_sub(document.caret().head() as u32)
                    .unwrap_or(0) as u32,
            ));
        // scroll down if cursor below visible document
        } else if last_index.start < document.caret().head() {
            warn!("last line {:?}", line_start);
            warn!("scroll_offset {:?}", document.scroll_offset());
            let ret = ViewMessage::ScrollDown(
                (line_start
                    .scroll_down()
                    .unwrap_or(&document.scroll_offset()))
                .checked_sub(document.scroll_offset())
                .unwrap_or(0),
            );
            if ret == ViewMessage::ScrollDown(0) {
                return None;
            }
            return Some(ret);
        }
        None
    }
    fn render_code(
        document: &Document,
        buffer: &mut Bitmap,
        normal: Font,
        keyword: Font,
        string: Font,
        number: Font,
        comment: Font,
    ) -> Option<ViewMessage> {
        buffer.clear(normal.bg_color);
        let input = document.text_buffer().to_string();
        let mut position = Vertex::zero();
        let mut rest: &str = &input;
        let keywords = &["int", "return", "for", "if", "while", "unsigned", "long"];
        while let Ok((new_rest, token)) = parse_clike(rest, keywords) {
            rest = new_rest;
            match token.get() {
                Token::Identifier(s) | Token::Operator(s) | Token::Whitespace(s) => {
                    (position, _) =
                        View::render_string(&String::from(*s), buffer, normal, position, None);
                }
                Token::Keyword(s) => {
                    (position, _) =
                        View::render_string(&String::from(*s), buffer, keyword, position, None);
                }
                Token::Number(s) => {
                    (position, _) =
                        View::render_string(&String::from(*s), buffer, number, position, None);
                }
                Token::String(s) => {
                    (position, _) =
                        View::render_string(&String::from(*s), buffer, string, position, None);
                }
                Token::Comment(s) => {
                    (position, _) =
                        View::render_string(&String::from(*s), buffer, comment, position, None);
                }
                Token::Punctuation(c) | Token::Other(c) => {
                    (position, _) =
                        View::render_string(&String::from(*c), buffer, normal, position, None);
                }
            }
        }

        None
    }
    pub fn render(document: &Document, buffer: &mut Bitmap) -> Option<ViewMessage> {
        let ret = match *document.view_config() {
            ViewConfig::Simple {
                font_scale,
                fg_color,
                bg_color,
            } => View::render_simple(document, buffer, font_scale, fg_color, bg_color),
            ViewConfig::Markdown {
                normal,
                emphasis,
                strong,
            } => View::render_markdown(document, buffer, normal, emphasis, strong),
            ViewConfig::Code {
                normal,
                keyword,
                string,
                number,
                comment,
            } => View::render_code(document, buffer, normal, keyword, string, number, comment),
        };
        warn!("scroll: {:?}", ret);
        return ret;
    }
}
