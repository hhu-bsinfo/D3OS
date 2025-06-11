use super::runnable::Runnable;
use crate::apps::text_editor::view::View;
use crate::{api::Command, WindowManager};
use alloc::collections::VecDeque;
use alloc::{boxed::Box, rc::Rc, string::String, vec};
use drawer::{rect_data::RectData, vertex::Vertex};
use font::Font;
use graphic::color::{Color, WHITE};
use graphic::lfb::DEFAULT_CHAR_WIDTH;
use graphic::{bitmap::Bitmap, lfb::DEFAULT_CHAR_HEIGHT};
use meassages::{Message, ViewMessage};
use model::{Document, ViewConfig};
use spin::rwlock::RwLock;
use terminal::DecodedKey;
use text_buffer::TextBuffer;

mod font;
mod meassages;
mod model;
mod view;

// Julius Drodofsky
pub struct TextEditor;

#[derive(Debug, Clone, Copy)]
pub struct TextEditorConfig {
    pub width: usize,
    pub height: usize,
    pub background_color: Color,
    pub markdown_view: ViewConfig,
    pub simple_view: ViewConfig,
}

impl TextEditorConfig {
    pub fn new(width: usize, height: usize) -> Self {
        let bg_color = Color::new(20, 20, 20, 255);
        let normal = Font {
            scale: 1,
            fg_color: WHITE,
            bg_color: bg_color,
            char_width: DEFAULT_CHAR_WIDTH,
            char_height: DEFAULT_CHAR_HEIGHT,
        };
        let strong = Font {
            scale: 1,
            fg_color: Color::new(69, 133, 136, 255),
            bg_color: bg_color,
            char_width: DEFAULT_CHAR_WIDTH,
            char_height: DEFAULT_CHAR_HEIGHT,
        };
        let emphasis = Font {
            scale: 1,
            fg_color: Color::new(131, 165, 152, 255),
            bg_color: bg_color,
            char_width: DEFAULT_CHAR_WIDTH,
            char_height: DEFAULT_CHAR_HEIGHT,
        };
        let markdown_view = ViewConfig::Markdown {
            normal: normal,
            emphasis: emphasis,
            strong: strong,
        };

        let simple_view = ViewConfig::Simple {
            font_scale: normal.scale,
            fg_color: normal.fg_color,
            bg_color: normal.bg_color,
        };
        TextEditorConfig {
            width: width,
            height: height,
            background_color: bg_color,
            markdown_view: markdown_view,
            simple_view: simple_view,
        }
    }
}

impl Runnable for TextEditor {
    fn run() {
        let config = TextEditorConfig::new(720, 500);
        let bitmap = Bitmap {
            width: config.width as u32,
            height: config.height as u32,
            data: vec![config.background_color; config.width * config.height],
        };
        let deque = VecDeque::<DecodedKey>::new();
        let handle = concurrent::thread::current()
            .expect("Failed to get thread")
            .id();
        let api = WindowManager::get_api();
        let canvas = Rc::new(RwLock::new(bitmap));
        let input = Rc::new(RwLock::<VecDeque<DecodedKey>>::new(deque));
        let input_clone = Rc::clone(&input);
        let component = api
            .execute(
                handle,
                None,
                Command::CreateCanvas {
                    styling: None,
                    rect_data: RectData {
                        top_left: Vertex::new(50, 80),
                        width: config.width as u32,
                        height: config.height as u32,
                    },
                    input: Some(Box::new(move |c: DecodedKey| {
                        input_clone.write().push_back(c);
                    })),
                    buffer: Rc::clone(&canvas),
                },
            )
            .unwrap();
        let markdown_example = r#"
# Heading 1

## Heading 2

This is a paragraph with **bold text** and *italic text*.

---

Another paragraph after a horizontal rule.

Some **Strong** Text.

Some *Emphasis* Text.

### Heading3

- Unordered item 1  
- Unordered item 2  
  - Nested unordered item  
  - Another nested item  

1. Ordered item 1  
2. Ordered item 2  
   1. Nested ordered item  
   2. Another nested item
"#;
        let text_buffer = TextBuffer::from_str(markdown_example);
        let mut document: Document =
            Document::new(Some(String::from("scratch")), text_buffer, config);

        document.update(meassages::Message::ViewMessage(ViewMessage::ScrollDown(0)));
        View::render(&document, &mut canvas.write());
        component.write().mark_dirty();
        let mut dirty = false;
        loop {
            let mut tmp_queue = VecDeque::<Message>::new();
            while let Some(value) = input.write().pop_front() {
                tmp_queue.push_back(Message::DecodedKey(value));
                dirty = true;
            }
            while let Some(value) = tmp_queue.pop_front() {
                document.update(value);
            }
            if dirty {
                {
                    // extra block to release canvas lock bevore calling mark_dirty
                    let mut msg = View::render(&document, &mut canvas.write());
                    while msg.is_some() {
                        document.update(meassages::Message::ViewMessage(msg.unwrap()));
                        msg = View::render(&document, &mut canvas.write());
                    }
                }
                {
                    component.write().mark_dirty()
                };
                dirty = false;
            }
        }
    }
}
