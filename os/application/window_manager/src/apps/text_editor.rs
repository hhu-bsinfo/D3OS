use alloc::{boxed::Box, rc::Rc, string::String, vec};
use crate::{alloc::string::ToString, components::component::ComponentStylingBuilder, config, signal::Signal};
use graphic::color::Color;
use spin::rwlock::RwLock;
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::{bitmap::Bitmap, lfb::DEFAULT_CHAR_HEIGHT};
use super::runnable::Runnable;
use crate::{api::Command, WindowManager};
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
        let handle = concurrent::thread::current().expect("Failed to get thread").id();
        let api = WindowManager::get_api();
        let canvas = Rc::new(RwLock::new(bitmap));
        let component = api.execute(handle, None,  Command::CreateCanvas { styling: None,  rect_data: RectData {
                    top_left: Vertex::new(50, 80),
                    width: config.widht as u32,
                    height: config.height as u32,
                },
        buffer: Rc::clone(&canvas) }).unwrap();


    }
}