use core::fmt::Debug;

use alloc::{boxed::Box, rc::Rc, string::String};
use concurrent::thread;
use drawer::drawer::{RectData, Vertex};
use hashbrown::HashMap;
use nolock::queues::mpsc::jiffy::{Receiver, Sender};
use spin::{Mutex, RwLock};

use crate::{
    apps::{clock::Clock, runnable::Runnable, test_app::TestApp},
    components::{button::Button, component::Component, dynamic_label::DynamicLabel},
};

extern crate alloc;

/// Default app to be used on startup of a new workspace
pub static DEFAULT_APP: &str = "clock";

enum FontScalingStrategy {
    /// We merely scale x and y values of the font
    WidthAndHeight,
    /**
    We scale by the smaller one of x and y and change
    the other in a way to keep the x/y ratio
    */
    RespectRatio,
}

pub enum Command {
    CreateButton {
        rel_rect_data: RectData,
        label: Option<Rc<Mutex<String>>>,
        on_click: Box<dyn Fn() -> ()>,
    },
    CreateDynamicLabel {
        rel_rect_data: RectData,
        text: Rc<RwLock<String>>,
        /// Function to be called on each window-manager main-loop iteration
        on_loop_iter: Option<Box<dyn Fn() -> ()>>,
        font_size: Option<u32>,
    },
}

pub struct Senders {
    pub tx_components: Sender<NewCompData>,
    pub tx_on_loop_iter: Sender<NewLoopIterFnData>,
}

pub struct Receivers {
    pub rx_components: Receiver<NewCompData>,
    pub rx_on_loop_iter: Receiver<NewLoopIterFnData>,
}

/**
API offers an interface between the window-manager and external programs that request
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
    /// absolute position on the screen
    abs_pos: RectData,
    ratios: (f64, f64),
}

#[derive(Clone, Copy, Debug)]
pub struct WindowData {
    pub workspace_index: usize,
    pub window_id: usize,
}

pub struct NewCompData {
    pub window_data: WindowData,
    pub component: Box<dyn Component>,
}

pub struct NewLoopIterFnData {
    pub window_data: WindowData,
    pub fun: Box<dyn Fn() -> ()>,
}

impl Debug for NewCompData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NewCompData")
            .field("window_data", &self.window_data)
            .finish()
    }
}

impl Debug for NewLoopIterFnData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NewLoopIterFnData")
            .field("window_data", &self.window_data)
            .finish()
    }
}

impl Api {
    pub fn new(screen_dims: (u32, u32), senders: Senders) -> Self {
        Self {
            handles: HashMap::new(),
            screen_dims,
            senders,
        }
    }

    /* Returning `Result<(), &str>` would make more sense, but
    I get a dumb borrow-checker error when I do so, thus we using `Option<()>` */
    pub fn register(
        &mut self,
        workspace_index: usize,
        window_id: usize,
        abs_pos: RectData,
        app_string: &str,
    ) -> Option<()> {
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

        Some(())
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
                rel_rect_data,
                label,
                on_click,
            } => {
                let abs_rect_data = self.scale_rect_to_window(rel_rect_data, handle_data);

                let button = Button::new(
                    handle_data.workspace_index,
                    abs_rect_data,
                    rel_rect_data,
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
                rel_rect_data,
                text,
                on_loop_iter,
                font_size,
            } => {
                let abs_rect_data = self.scale_rect_to_window(rel_rect_data, handle_data);
                let scaled_font_size = font_size.map(|original_size| {
                    self.scale_font_to_window(
                        original_size,
                        &handle_data.ratios,
                        FontScalingStrategy::RespectRatio,
                    )
                });

                let label = DynamicLabel::new(
                    handle_data.workspace_index,
                    abs_rect_data.top_left,
                    rel_rect_data.top_left,
                    text,
                    scaled_font_size,
                );

                let dispatch_data = NewCompData {
                    window_data,
                    component: Box::new(label),
                };

                self.add_component(dispatch_data);

                if let Some(fun) = on_loop_iter {
                    let data = NewLoopIterFnData { window_data, fun };
                    self.add_on_loop_iter_fn(data);
                }
            }
        }

        Ok(())
    }

    pub fn is_app_name_valid(&self, app_string: &str) -> bool {
        self.map_app_string_to_fn(app_string).is_some()
    }

    fn map_app_string_to_fn(&self, app_string: &str) -> Option<fn()> {
        match app_string {
            "clock" => Some(Clock::run),
            "test_app" => Some(TestApp::run),
            _ => None,
        }
    }

    fn add_component(&self, dispatch_data: NewCompData) {
        self.senders
            .tx_components
            .enqueue(dispatch_data)
            .expect("components queue was closed!");
    }

    fn add_on_loop_iter_fn(&self, fun: NewLoopIterFnData) {
        self.senders
            .tx_on_loop_iter
            .enqueue(fun)
            .expect("on_loop_iter queue was closed!");
    }

    fn scale_rect_to_window(
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

    #[allow(unused_variables, unreachable_code)]
    fn scale_font_to_window(
        &self,
        original_font_size: u32,
        ratios: &(f64, f64),
        strategy: FontScalingStrategy,
    ) -> (u32, u32) {
        /* TODO: Fix font scaling not working properly. It scaled fonts up, instead of down.
        A working user-mode debugger would really be helpful right now, huh? */
        return (1, 1);
        let float_font_size = f64::from(original_font_size);
        match strategy {
            FontScalingStrategy::WidthAndHeight => (
                ((float_font_size * ratios.0) as u32).max(1),
                ((float_font_size * ratios.1) as u32).max(1),
            ),
            FontScalingStrategy::RespectRatio => {
                let min_ratio = ratios.0.min(ratios.1);
                let new_font = ((float_font_size * min_ratio) as u32).max(1);

                (new_font, new_font)
            }
        }
    }
}
