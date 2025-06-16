use super::runnable::Runnable;
use crate::apps::text_editor::view::View;
use crate::{api::Command, WindowManager};
use alloc::{boxed::Box, rc::Rc, string::String, vec};
use drawer::{rect_data::RectData, vertex::Vertex};
use font::Font;
use graphic::color::{Color, WHITE};
use graphic::lfb::DEFAULT_CHAR_WIDTH;
use graphic::{
    bitmap::{Bitmap, ScalingMode},
    lfb::DEFAULT_CHAR_HEIGHT,
};
use meassages::Message;
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

fn handle_keyboard_input(
    document: &Rc<RwLock<Document>>,
    canvas: &Rc<RwLock<Bitmap>>,
    key: DecodedKey,
) {
    document.write().update(Message::DecodedKey(key));
    let mut msg = View::render(&document.read(), &mut canvas.write());
    while msg.is_some() {
        document.write().update(Message::ViewMessage(msg.unwrap()));
        msg = View::render(&document.read(), &mut canvas.write());
    }
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
        let config = TextEditorConfig::new(900, 600);
        let bitmap = Bitmap {
            width: (0.7 * (config.width as f32)) as u32,
            height: (0.7 * (config.height as f32)) as u32,
            data: vec![config.background_color; config.width * config.height],
        };
        let handle = concurrent::thread::current()
            .expect("Failed to get thread")
            .id();
        let api = WindowManager::get_api();
        let canvas = Rc::new(RwLock::new(bitmap));
        let canvs_clone = Rc::clone(&canvas);

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
        let mut document = Document::new(Some(String::from("scratch")), text_buffer, config);
        View::render(&document, &mut canvas.write());

        let model = Rc::new(RwLock::<Document<'_>>::new(document));
        let _component = api
            .execute(
                handle,
                None,
                Command::CreateCanvas {
                    styling: None,
                    log_rect_data: RectData {
                        top_left: Vertex::new(50, 50),
                        width: config.width as u32,
                        height: config.height as u32,
                    },
                    input: Some(Box::new(move |c: DecodedKey| {
                        handle_keyboard_input(&model, &canvs_clone, c);
                    })),
                    buffer: Rc::clone(&canvas),
                    scaling_mode: ScalingMode::Bilinear,
                },
            )
            .unwrap();
    }
}
