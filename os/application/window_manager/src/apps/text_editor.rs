use super::runnable::Runnable;
use crate::api::LOG_SCREEN;
use crate::apps::text_editor::config::TextEditorConfig;
use crate::apps::text_editor::editor::OpenDocuments;
use crate::apps::text_editor::messages::Message;
use crate::apps::text_editor::view::View;
use crate::components::component::ComponentStyling;
use crate::components::container::basic_container::{AlignmentMode, LayoutMode, StretchMode};
use crate::components::container::ContainerStyling;
use crate::config::DEFAULT_BACKGROUND_COLOR;
use crate::signal::{ComponentRef, Signal};
use crate::{api::Command, WindowManager};
use alloc::{boxed::Box, rc::Rc, string::String, vec};
use drawer::{rect_data::RectData, vertex::Vertex};
use editor::apply_message;
use graphic::bitmap::{Bitmap, ScalingMode};
use spin::rwlock::RwLock;
use terminal::DecodedKey;

mod config;
mod editor;
mod font;
mod messages;
mod model;
mod view;

// Julius Drodofsky

pub struct TextEditor;

impl Runnable for TextEditor {
    fn run() {
        let config =
            TextEditorConfig::new(LOG_SCREEN.0 as usize - 100, LOG_SCREEN.1 as usize - 40, &[]);
        let bitmap = Bitmap {
            width: config.width as u32,
            height: config.height as u32,
            data: vec![config.background_color; config.width * config.height],
        };
        let handle = concurrent::thread::current()
            .expect("Failed to get thread")
            .id();
        let api = WindowManager::get_api();
        let canvas = Rc::new(RwLock::new(bitmap));
        let canvs_clone = Rc::clone(&canvas);
        let edit_canvas: Rc<RwLock<Option<ComponentRef>>> = Rc::new(RwLock::new(None));
        let mut documents = OpenDocuments::dummy();
        View::render(documents.current().unwrap(), &mut canvas.write());
        let mut container_styling = ContainerStyling::default();
        let curret_file = Signal::new(documents.current().unwrap().path().unwrap());
        container_styling.show_border = false;
        container_styling.maintain_aspect_ratio = false;
        container_styling.child_padding = 2;
        container_styling.background_color = DEFAULT_BACKGROUND_COLOR;
        container_styling.show_background = true;
        let label_styling = ComponentStyling::default();
        let model = Rc::new(RwLock::<OpenDocuments<'_, '_>>::new(documents));
        let _parent_container = api
            .execute(
                handle,
                None,
                Command::CreateContainer {
                    log_rect_data: RectData {
                        top_left: Vertex { x: 50, y: 50 },
                        width: LOG_SCREEN.0 - 100,
                        height: LOG_SCREEN.1 - 140,
                    },
                    layout: LayoutMode::Vertical(AlignmentMode::Top),
                    stretch: StretchMode::Fill,
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
                        width: LOG_SCREEN.0 as u32 - 100,
                        height: 40,
                    },
                    layout: LayoutMode::Horizontal(AlignmentMode::Top),
                    stretch: StretchMode::Fill,
                    styling: Some(container_styling),
                },
            )
            .expect("failed to create container");

        let _current_file = api.execute(
            handle,
            Some(_menu_container.clone()),
            Command::CreateLabel {
                log_pos: Vertex::new(0, 0),
                text: Rc::clone(&curret_file),
                on_loop_iter: None,
                font_size: Some(1),
                styling: Some(label_styling),
            },
        );

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
                    apply_message(
                        &model_clone,
                        &Rc::clone(&canvas_clone),
                        Message::CommandMessage(messages::CommandMessage::Undo),
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
                    apply_message(
                        &model_clone,
                        &Rc::clone(&canvas_clone),
                        Message::CommandMessage(messages::CommandMessage::Redo),
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
                    apply_message(
                        &model_clone,
                        &Rc::clone(&canvas_clone),
                        Message::CommandMessage(messages::CommandMessage::Markdown),
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

        let _code = api.execute(
            handle,
            Some(_menu_container.clone()),
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(0, 0),
                    width: 140,
                    height: 60,
                },
                label: Some((Signal::new(String::from("Code")), 1)),
                on_click: Some(Box::new(move || {
                    apply_message(
                        &model_clone,
                        &Rc::clone(&canvas_clone),
                        Message::CommandMessage(messages::CommandMessage::CLike),
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
        let current_file_clone = Rc::clone(&curret_file);
        let _prev = api.execute(
            handle,
            Some(_menu_container.clone()),
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(0, 0),
                    width: 60,
                    height: 60,
                },
                label: Some((Signal::new(String::from("<")), 1)),
                on_click: Some(Box::new(move || {
                    {
                        let mut models = model_clone.write();
                        let prev = models.prev();
                        if prev.is_some() {
                            current_file_clone
                                .set(prev.unwrap().path().unwrap_or(String::from("scratch")));
                        }
                    }
                    apply_message(
                        &model_clone,
                        &Rc::clone(&canvas_clone),
                        Message::CommandMessage(messages::CommandMessage::None),
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
        let current_file_clone = Rc::clone(&curret_file);
        let _next = api.execute(
            handle,
            Some(_menu_container.clone()),
            Command::CreateButton {
                log_rect_data: RectData {
                    top_left: Vertex::new(0, 0),
                    width: 60,
                    height: 60,
                },
                label: Some((Signal::new(String::from(">")), 1)),
                on_click: Some(Box::new(move || {
                    {
                        let mut models = model_clone.write();
                        let next = models.next();
                        if next.is_some() {
                            current_file_clone
                                .set(next.unwrap().path().unwrap_or(String::from("scratch")));
                        }
                    }
                    apply_message(
                        &model_clone,
                        &Rc::clone(&canvas_clone),
                        Message::CommandMessage(messages::CommandMessage::None),
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
                        apply_message(&model, &canvs_clone, Message::DecodedKey(c));
                    })),
                    buffer: Rc::clone(&canvas),
                    scaling_mode: ScalingMode::None,
                },
            )
            .unwrap(),
        );
    }
}
