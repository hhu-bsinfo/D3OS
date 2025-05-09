use crate::{alloc::string::ToString, components::component::ComponentStylingBuilder, signal::Signal};
use alloc::{boxed::Box, rc::Rc, string::String, vec};
use drawer::vertex::Vertex;
use spin::rwlock::RwLock;

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

pub struct CanvasApp;

impl Runnable for CanvasApp {
    fn run() {
        let handle = concurrent::thread::current().expect("Failed to get thread").id();
        let api = WindowManager::get_api();
        let buffer = Rc::new(RwLock::new(vec![0u32; 500 * 200]));
        let component = api.execute(handle, None,  Command::CreateCanvas { styling: None, width: 500, height: 200, buffer: Rc::clone(&buffer) }).unwrap();
       
    }
}
