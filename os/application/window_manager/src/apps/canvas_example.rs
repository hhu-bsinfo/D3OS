use crate::{alloc::string::ToString, components::component::ComponentStylingBuilder, signal::Signal};
use alloc::{boxed::Box, rc::Rc, string::String, vec};
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::bitmap::Bitmap;
use graphic::color::Color;
use spin::rwlock::RwLock;

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

// Julius Drodofsky
pub struct CanvasApp;

impl Runnable for CanvasApp {
    fn run() {
        let bitmap_red = Bitmap {
            width: 10,
            height: 10,
            data: vec![
                Color { red: 255, green: 0, blue: 0, alpha: 255 }; // 10x10 rote Pixel
                100 // 10 * 10
            ],
        };
        let handle = concurrent::thread::current().expect("Failed to get thread").id();
        let api = WindowManager::get_api();
        let buffer = Rc::new(RwLock::new(bitmap_red));
        let component = api.execute(handle, None,  Command::CreateCanvas { styling: None,  rect_data: RectData {
                    top_left: Vertex::new(50, 50),
                    width: 50,
                    height: 50,
                },
        buffer: Rc::clone(&buffer) }).unwrap();
    }
}
