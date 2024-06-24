use core::fmt::Debug;

use alloc::{boxed::Box, rc::Rc, string::String};
use concurrent::thread;
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH};
use hashbrown::HashMap;
use nolock::queues::mpsc::jiffy::{Receiver, Sender};
use spin::{Mutex, RwLock};

use crate::{
    apps::{clock::Clock, runnable::Runnable, test_app::TestApp},
    components::{
        button::Button, component::Component, dynamic_label::DynamicLabel, input_field::InputField,
    },
    configs::general::PADDING_BORDERS_AND_CHARS,
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
        rel_pos: Vertex,
        text: Rc<RwLock<String>>,
        /// Function to be called on each window-manager main-loop iteration
        on_loop_iter: Option<Box<dyn Fn() -> ()>>,
        font_size: Option<usize>,
    },
    CreateInputField {
        /// The maximum amount of chars to fit in this field
        width_in_chars: usize,
        font_size: Option<usize>,
        rel_pos: Vertex,
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
    /// handles are equal to the thread-id of the application
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
                f64::from(abs_pos.width) / f64::from(self.screen_dims.0),
                f64::from(abs_pos.height) / f64::from(self.screen_dims.1),
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
                rel_pos,
                text,
                on_loop_iter,
                font_size,
            } => {
                let font_size = font_size.unwrap_or(1);
                let scaled_font_scale = self.scale_font_to_window(
                    font_size,
                    &handle_data.ratios,
                    FontScalingStrategy::RespectRatio,
                );

                let scaled_pos = self.scale_vertex_to_window(rel_pos, handle_data);

                let text_rc = Rc::clone(&text);

                let label = DynamicLabel::new(
                    handle_data.workspace_index,
                    scaled_pos,
                    rel_pos,
                    text_rc,
                    scaled_font_scale,
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
            Command::CreateInputField {
                rel_pos,
                width_in_chars,
                font_size,
            } => {
                let font_size = font_size.unwrap_or(1);
                let scaled_font_scale = self.scale_font_to_window(
                    font_size,
                    &handle_data.ratios,
                    FontScalingStrategy::RespectRatio,
                );

                let scaled_pos = self.scale_vertex_to_window(rel_pos, handle_data);
                let rel_rect_data = RectData {
                    top_left: rel_pos,
                    width: DEFAULT_CHAR_WIDTH * (font_size * width_in_chars) as u32,
                    height: DEFAULT_CHAR_HEIGHT * font_size as u32,
                };
                let abs_rect_data = RectData {
                    top_left: scaled_pos,
                    width: DEFAULT_CHAR_WIDTH * scaled_font_scale.0 * width_in_chars as u32
                        + PADDING_BORDERS_AND_CHARS * 2,
                    height: DEFAULT_CHAR_HEIGHT * scaled_font_scale.1,
                };

                let input_field = InputField::new(
                    handle_data.workspace_index,
                    abs_rect_data,
                    rel_rect_data,
                    width_in_chars,
                );

                let dispatch_data = NewCompData {
                    window_data,
                    component: Box::new(input_field),
                };

                self.add_component(dispatch_data);
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
        original_font_size: usize,
        ratios: &(f64, f64),
        strategy: FontScalingStrategy,
    ) -> (u32, u32) {
        /* TODO: Fix font scaling not working properly. It scaled fonts up, instead of down.
        A working user-mode debugger would really be helpful right now, huh? */
        return (1, 1);
        let float_font_size = f64::from(original_font_size as u32);
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

    fn scale_vertex_to_window(
        &self,
        original_vert: Vertex,
        HandleData {
            abs_pos, ratios, ..
        }: &HandleData,
    ) -> Vertex {
        let new_x = (f64::from(original_vert.x) * ratios.0 + f64::from(abs_pos.top_left.x)) as u32;
        let new_y = (f64::from(original_vert.y) * ratios.1 + f64::from(abs_pos.top_left.y)) as u32;

        return Vertex::new(new_x, new_y);
    }
}
