use alloc::{borrow::ToOwned, boxed::Box, rc::Rc, string::String, vec};
use model::Document;
use terminal::DecodedKey;
use crate::{alloc::string::ToString, components::component::ComponentStylingBuilder, config, signal::Signal};
use graphic::{buffered_lfb, color::{Color, WHITE}};
use spin::rwlock::RwLock;
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::{bitmap::Bitmap, lfb::DEFAULT_CHAR_HEIGHT};
use super::runnable::Runnable;
use crate::{api::Command, WindowManager};
use crate::apps::text_editor::view::View;
use text_buffer::TextBuffer;
use alloc::collections::VecDeque;

mod view;
mod model;

// Julius Drodofsky
pub struct TextEditor;

pub struct TextEditorConfig {
    pub widht: usize,
    pub height: usize,
    pub background_color: Color,
}


impl Runnable for TextEditor {
    fn run() {
        let config = TextEditorConfig{widht: 720, height: 500, background_color: Color::new(20,20,20,255)};
        let bitmap = Bitmap {
            width: config.widht as u32,
            height: config.height as u32,
            data: vec![
                config.background_color;
                config.widht*config.height
            ],
        };
        let deque = VecDeque::<DecodedKey>::new();
        let handle = concurrent::thread::current().expect("Failed to get thread").id();
        let api = WindowManager::get_api();
        let canvas = Rc::new(RwLock::new(bitmap));
        let input = Rc::new(RwLock::<VecDeque<DecodedKey>>::new(deque));
        let input_clone = Rc::clone(&input);
        let component = api.execute(handle, None,  Command::CreateCanvas { styling: None,  rect_data: RectData {
                    top_left: Vertex::new(50, 80),
                    width: config.widht as u32,
                    height: config.height as u32,
                },
                input: Some(Box::new(move |c: DecodedKey| {
                    input_clone.write().push_back(c);
                })),
                buffer: Rc::clone(&canvas),
            }).unwrap();
        

        let mut text_buffer = TextBuffer::from_str("Das ist ein Text!");
        let mut document: Document = Document::new(Some(String::from("scratch")), text_buffer);
        
        let mut view = View::Simple{font_scale: 1, fg_color: WHITE, bg_color: config.background_color};
        view.render(&document, &mut canvas.write());
        component.write().mark_dirty();
        let mut dirty = false;
        loop {
            while let Some(value) = input.write().pop_front(){
                document.update(value); 
                view.render(&document, &mut canvas.write());
                dirty = true;
            }
            if dirty {
                component.write().mark_dirty();
                dirty = false;
            }   
        }
    }
}