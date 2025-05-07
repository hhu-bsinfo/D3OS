use crate::{alloc::string::ToString, components::component::ComponentStylingBuilder, signal::Signal};
use alloc::{boxed::Box, rc::Rc, string::String};
use drawer::vertex::Vertex;


use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

pub struct CanvasApp;

impl Runnable for CanvasApp {
    fn run() {
        let handle = concurrent::thread::current().expect("Failed to get thread").id();
        let api = WindowManager::get_api();

       api.execute(handle, Command::CreateCanvas { styling: None, width: 500, height: 200 });
    }
}
