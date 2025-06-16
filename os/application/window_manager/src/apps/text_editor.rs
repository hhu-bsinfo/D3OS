use super::runnable::Runnable;
use crate::api::LOG_SCREEN;
use crate::apps::text_editor::view::View;
use crate::components::container::basic_container::{AlignmentMode, LayoutMode, StretchMode};
use crate::components::container::ContainerStyling;
use crate::signal::{ComponentRef, Signal};
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
use logger::warn;
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

fn render_msg(document: &Rc<RwLock<Document>>, canvas: &Rc<RwLock<Bitmap>>, msg: Message) {
    document.write().update(msg);
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
        let edit_canvas: Rc<RwLock<Option<ComponentRef>>> = Rc::new(RwLock::new(None));
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
        let mut container_styling = ContainerStyling::default();
        container_styling.show_border = false;
        container_styling.maintain_aspect_ratio = false;
        container_styling.child_padding = 2;
        let model = Rc::new(RwLock::<Document<'_>>::new(document));
        let _parent_container = api
            .execute(
                handle,
                None,
                Command::CreateContainer {
                    log_rect_data: RectData {
                        top_left: Vertex { x: 50, y: 50 },
                        width: LOG_SCREEN.0,
                        height: LOG_SCREEN.1,
                    },
                    layout: LayoutMode::Vertical(AlignmentMode::Top),
                    stretch: StretchMode::None,
                    styling: Some(container_styling),
                },
            )
            .expect("failed to create container");
        let _menu_container = api
            .execute(
                handle,
                Some(_parent_container.clone()),
                Command::CreateContainer {
                    log_rect_data: RectData {
                        top_left: Vertex { x: 0, y: 0 },
                        width: LOG_SCREEN.0 as u32,
                        height: 60,
                    },
                    layout: LayoutMode::Horizontal(AlignmentMode::Top),
                    stretch: StretchMode::Fill,
                    styling: Some(container_styling),
                },
            )
            .expect("failed to create container");

        let model_clone = Rc::clone(&model);
        let canvas_clone = Rc::clone(&canvas);
        let edit_canvas_clone = Rc::clone(&edit_canvas);

        let _undo = api.execute(
            handle,
            Some(_menu_container.clone()),
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(0, 0),
                    width: 100,
                    height: 60,
                },
                label: Some((Signal::new(String::from("Undo")), 0)),
                on_click: Some(Box::new(move || {
                    render_msg(
                        &model_clone,
                        &Rc::clone(&canvas_clone),
                        Message::CommandMessage(meassages::CommandMessage::Undo),
                    );
                    edit_canvas_clone
                        .write()
                        .as_ref()
                        .unwrap()
                        .write()
                        .mark_dirty();
                })),
                styling: None,
            },
        );

        let model_clone = Rc::clone(&model);
        let canvas_clone = Rc::clone(&canvas);
        let edit_canvas_clone = Rc::clone(&edit_canvas);

        let _redo = api.execute(
            handle,
            Some(_menu_container.clone()),
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(0, 0),
                    width: 100,
                    height: 60,
                },
                label: Some((Signal::new(String::from("Redo")), 1)),
                on_click: Some(Box::new(move || {
                    render_msg(
                        &model_clone,
                        &Rc::clone(&canvas_clone),
                        Message::CommandMessage(meassages::CommandMessage::Redo),
                    );
                    edit_canvas_clone
                        .write()
                        .as_ref()
                        .unwrap()
                        .write()
                        .mark_dirty();
                })),
                styling: None,
            },
        );
        let model_clone = Rc::clone(&model);
        let canvas_clone = Rc::clone(&canvas);
        let edit_canvas_clone = Rc::clone(&edit_canvas);

        let _markdown = api.execute(
            handle,
            Some(_menu_container.clone()),
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(0, 0),
                    width: 240,
                    height: 60,
                },
                label: Some((Signal::new(String::from("MD - Preview")), 1)),
                on_click: Some(Box::new(move || {
                    render_msg(
                        &model_clone,
                        &Rc::clone(&canvas_clone),
                        Message::CommandMessage(meassages::CommandMessage::Markdown),
                    );
                    edit_canvas_clone
                        .write()
                        .as_ref()
                        .unwrap()
                        .write()
                        .mark_dirty();
                })),
                styling: None,
            },
        );

        *edit_canvas.write() = Some(
            api.execute(
                handle,
                Some(_parent_container.clone()),
                Command::CreateCanvas {
                    styling: None,
                    log_rect_data: RectData {
                        top_left: Vertex::new(0, 0),
                        width: config.width as u32,
                        height: config.height as u32,
                    },
                    input: Some(Box::new(move |c: DecodedKey| {
                        render_msg(&model, &canvs_clone, Message::DecodedKey(c));
                    })),
                    buffer: Rc::clone(&canvas),
                    scaling_mode: ScalingMode::Bilinear,
                },
            )
            .unwrap(),
        );
    }
}
