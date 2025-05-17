use crate::{alloc::string::ToString, components::component::ComponentStylingBuilder, signal::Signal};
use alloc::collections::vec_deque::VecDeque;
use alloc::{boxed::Box, rc::Rc, string::String, vec};
use concurrent::thread::sleep;
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::lfb::DEFAULT_CHAR_WIDTH;
use graphic::{bitmap::Bitmap, lfb::DEFAULT_CHAR_HEIGHT};
use graphic::color::Color;
use spin::rwlock::RwLock;
use terminal::println;
use terminal::print;

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

// Julius Drodofsky
pub struct CanvasApp;

impl Runnable for CanvasApp {
    fn run() {
        //initialise values for component
        let bitmap_red = Bitmap {
            width: 200,
            height: 100,
            data: vec![
                Color { red: 255, green: 0, blue: 0, alpha: 255 }; // 10x10 rote Pixel
                20000 // 10 * 10
            ],
        };
        let deque = VecDeque::<char>::new();
        let handle = concurrent::thread::current().expect("Failed to get thread").id();
        let api = WindowManager::get_api();
        let canvas = Rc::new(RwLock::new(bitmap_red));
        let input = Rc::new(RwLock::<VecDeque<char>>::new(deque));
        let input_clone = Rc::clone(&input);
        //create component
        let component = api.execute(handle, None,  Command::CreateCanvas { styling: None,  rect_data: RectData {
                    top_left: Vertex::new(50, 80),
                    width: 200,
                    height: 100,
                },
        buffer: Rc::clone(&canvas),
        input: Some(Box::new(move |c: char| {
                    input_clone.write().push_back(c);
                })),
         }).unwrap();
        //use component
        let mut x = 0;
        x = canvas.write().draw_char_scaled(x, 0, 1, 1, Color::new(255, 255, 255, 255), Color::new(0, 0, 0, 50), 'R');
        canvas.write().draw_char_scaled(x, 0, 1, 1, Color::new(255, 255, 255, 255), Color::new(0, 0, 0, 50), 'o');
        canvas.write().draw_char_scaled(x*2, 0, 1, 1, Color::new(255, 255, 255, 255), Color::new(0, 0, 0, 50), 't');
        canvas.write().draw_line(0, DEFAULT_CHAR_HEIGHT, x*3, DEFAULT_CHAR_HEIGHT, Color::new(255, 255, 255, 255));
        component.write().mark_dirty();
        x=0;
        let mut y=DEFAULT_CHAR_HEIGHT;
        loop {
            while let Some(value) = input.write().pop_front(){
                if value == '\n' {
                    y+= DEFAULT_CHAR_HEIGHT;
                    x=0;
                    continue;
                }
                if canvas.read().width - x < DEFAULT_CHAR_WIDTH {
                    x=0;
                    y+= DEFAULT_CHAR_HEIGHT;
                }
                x+=canvas.write().draw_char_scaled(x, y, 1, 1, Color::new(255, 255, 255, 255), Color::new(0, 0, 0, 50), value);
                // Die Anwendung ist deutlich schneller wenn nicht nach jedem Buchstaben, sondern nur sobald die queue leer ist gezeichnet wird:)
                component.write().mark_dirty();
            }
        }
        
    }
}
