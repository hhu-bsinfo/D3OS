use alloc::{boxed::Box, rc::Rc, string::String};
use concurrent::thread;
use drawer::drawer::{RectData, Vertex};
use hashbrown::HashMap;
use nolock::queues::mpsc::jiffy::{Receiver, Sender};
use spin::{Mutex, RwLock};

use crate::{
    apps::{clock::Clock, runnable::Runnable, test_app::TestApp},
    components::{button::Button, component::Component, dynamic_label::DynamicLabel},
    WindowManager,
};

extern crate alloc;

pub enum Command {
    CreateButton {
        pos: RectData,
        label: Option<Rc<Mutex<String>>>,
        on_click: Box<dyn Fn() -> ()>,
    },
    CreateDynamicLabel {
        pos: RectData,
        text: Rc<RwLock<String>>,
        on_loop_iter: Option<Box<dyn Fn() -> ()>>,
    },
}

pub struct Senders {
    pub tx_components: Sender<NewCompData>,
    pub tx_on_loop_iter: Sender<NewLoopIterFunData>,
}

pub struct Receivers {
    pub rx_components: Receiver<NewCompData>,
    pub rx_on_loop_iter: Receiver<NewLoopIterFunData>,
}

/**
Api offers an interface between the window-manager and external programs that request
a service from the window-manager, like creating components or subscribing callback-functions to be
executed in the main-loop.
*/
pub struct Api {
    handles: HashMap<usize, HandleData>,
    screen_dims: (u32, u32),
    senders: Senders,
}

/// All information saved for a single handle
pub struct HandleData {
    workspace_index: usize,
    window_id: usize,
    abs_pos: RectData,
    ratios: (f64, f64),
}

#[derive(Clone, Copy)]
pub struct WindowData {
    pub workspace_index: usize,
    pub window_id: usize,
}

pub struct NewCompData {
    pub window_data: WindowData,
    pub component: Box<dyn Component>,
}

pub struct NewLoopIterFunData {
    pub window_data: WindowData,
    pub fun: Box<dyn Fn() -> ()>,
}

impl Api {
    pub fn new(screen_dims: (u32, u32), senders: Senders) -> Self {
        Self {
            handles: HashMap::new(),
            screen_dims,
            senders,
        }
    }

    pub fn register(
        &mut self,
        workspace_index: usize,
        window_id: usize,
        abs_pos: RectData,
        app_string: String,
    ) -> Option<usize> {
        let app_fn_ptr = self.map_app_string_to_fn(app_string)?;

        let handle = thread::create(app_fn_ptr).id();
        let handle_data = HandleData {
            workspace_index,
            window_id,
            abs_pos,
            ratios: (
                abs_pos.width as f64 / self.screen_dims.0 as f64,
                abs_pos.height as f64 / self.screen_dims.1 as f64,
            ),
        };

        self.handles.insert(handle, handle_data);

        return Some(handle);
    }

    pub fn execute(&self, handle: usize, command: Command) -> Result<(), &str> {
        let handle_data = self
            .handles
            .get(&handle)
            .ok_or("Provided handle not found")?;

        let window_data = WindowData {
            workspace_index: handle_data.workspace_index,
            window_id: handle_data.window_id,
        };

        match command {
            Command::CreateButton {
                pos,
                label,
                on_click,
            } => {
                let scaled_pos = self.scale_to_window(pos, handle_data);

                let button = Button::new(
                    WindowManager::generate_id(),
                    handle_data.workspace_index,
                    scaled_pos,
                    label,
                    on_click,
                );

                let dispatch_data = NewCompData {
                    window_data,
                    component: Box::new(button),
                };

                self.add_component(dispatch_data);
            }
            Command::CreateDynamicLabel {
                pos,
                text,
                on_loop_iter,
            } => {
                let scaled_pos = self.scale_to_window(pos, handle_data);

                let label = DynamicLabel::new(
                    WindowManager::generate_id(),
                    handle_data.workspace_index,
                    scaled_pos.top_left,
                    text,
                );

                let dispatch_data = NewCompData {
                    window_data,
                    component: Box::new(label),
                };

                self.add_component(dispatch_data);

                if let Some(fun) = on_loop_iter {
                    let data = NewLoopIterFunData { window_data, fun };
                    self.add_on_loop_iter_fun(data);
                }
            }
        }

        Ok(())
    }

    fn map_app_string_to_fn(&self, app_string: String) -> Option<fn()> {
        match app_string.as_str() {
            "clock" => Some(Clock::run),
            "test_app" => Some(TestApp::run),
            _ => None,
        }
    }

    fn add_component(&self, dispatch_data: NewCompData) {
        self.senders.tx_components.enqueue(dispatch_data);
    }

    fn add_on_loop_iter_fun(&self, fun: NewLoopIterFunData) {
        self.senders.tx_on_loop_iter.enqueue(fun);
    }

    fn scale_to_window(
        &self,
        RectData {
            top_left,
            width,
            height,
        }: RectData,
        HandleData {
            abs_pos, ratios, ..
        }: &HandleData,
    ) -> RectData {
        RectData {
            top_left: Vertex::new(
                (f64::from(top_left.x) * ratios.0) as u32 + abs_pos.top_left.x,
                (f64::from(top_left.y) * ratios.1) as u32 + abs_pos.top_left.y,
            ),
            width: (f64::from(width) * ratios.0) as u32,
            height: (f64::from(height) * ratios.1) as u32,
        }
    }
}
