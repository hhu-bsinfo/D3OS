use core::{fmt::Debug, usize};

use alloc::{boxed::Box, rc::Rc, string::String};
use concurrent::thread;
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::bitmap::{Bitmap, ScalingMode};
use hashbrown::HashMap;
use nolock::queues::mpsc::jiffy::{Receiver, Sender};
use spin::rwlock::RwLock;
use terminal::DecodedKey;

use crate::{
    apps::{
        bitmap_app::BitmapApp, calculator::Calculator, canvas_example::CanvasApp, clock::Clock,
        counter::Counter, layout_app::LayoutApp, radio_buttons::RadioButtonApp, runnable::Runnable,
        slider_app::SliderApp, submit_label::SubmitLabel, text_editor::TextEditor,
    },
    components::{
        bitmap::BitmapGraphic,
        button::Button,
        canvas::Canvas,
        checkbox::Checkbox,
        component,
        container::{
            basic_container::BasicContainer,
            container_layout::ContainerLayout,
            ContainerStyling,
        },
        input_field::InputField,
        label::Label,
        radio_button_group::RadioButtonGroup,
        slider::Slider,
    },
    signal::{ComponentRef, ComponentRefExt, Signal, Stateful},
    SCREEN,
};

use self::component::ComponentStyling;

extern crate alloc;

/// Default app to be used on startup of a new workspace
pub static DEFAULT_APP: &str = "editor";

/// Logical screen resolution, used by apps for describing component locations
pub const LOG_SCREEN: (u32, u32) = (1000, 750);

pub enum Command<'a> {
    CreateButton {
        log_rect_data: RectData,
        label: Option<(Rc<Signal<String>>, usize)>,
        on_click: Option<Box<dyn Fn() -> ()>>,
        styling: Option<ComponentStyling>,
    },
    CreateLabel {
        log_pos: Vertex,
        text: Rc<Signal<String>>,
        /**
        Function to be called on each window-manager main-loop iteration.
        If it returns true, the containing-window dirty-bit is set
        */
        on_loop_iter: Option<Box<dyn Fn() -> bool>>,
        font_size: Option<usize>,
        styling: Option<ComponentStyling>,
    },
    CreateInputField {
        /// The maximum amount of chars to fit in this field
        log_rect_data: RectData,
        // log_pos: Vertex,
        width_in_chars: usize,
        font_size: Option<usize>,
        starting_text: String,
        on_change: Option<Box<dyn Fn(String) -> ()>>,
        styling: Option<ComponentStyling>,
    },
    CreateCheckbox {
        log_rect_data: RectData,
        state: Stateful<bool>,
        on_change: Option<Box<dyn Fn(bool) -> ()>>,
        styling: Option<ComponentStyling>,
    },
    CreateBitmapGraphic {
        log_rect_data: RectData,
        bitmap: &'a Bitmap,
        scaling_mode: ScalingMode,
        styling: Option<ComponentStyling>,
    },
    CreateSlider {
        log_rect_data: RectData,
        on_change: Option<Box<dyn Fn(i32) -> ()>>,
        value: Stateful<i32>,
        min: i32,
        max: i32,
        steps: u32,
        styling: Option<ComponentStyling>,
    },
    CreateRadioButtonGroup {
        center: Vertex,
        radius: u32,
        spacing: u32,
        num_buttons: usize,
        // options: Vec<String>,
        selected_option: Stateful<usize>,
        on_change: Option<Rc<Box<dyn Fn(usize) -> ()>>>,
        styling: Option<ComponentStyling>,
    },
    CreateCanvas {
        styling: Option<ComponentStyling>,
        log_rect_data: RectData,
        buffer: Rc<RwLock<Bitmap>>,
        input: Option<Box<dyn Fn(DecodedKey) -> ()>>,
        scaling_mode: ScalingMode,
    },
    CreateContainer {
        log_rect_data: RectData,
        layout: Option<ContainerLayout>,
        styling: Option<ContainerStyling>,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum ScreenSplitType {
    Horizontal,
    Vertical,
}

#[derive(Debug)]
pub enum WindowManagerMessage {
    CreateNewWorkspace,
    CloseCurrentWorkspace,
    SwitchToWorkspace(usize),

    CloseCurrentWindow,
    MoveCurrentWindowForward,
    MoveCurrentWindowBackward,

    LaunchApp(String, ScreenSplitType),
}

pub struct Senders {
    pub tx_components: Sender<NewCompData>,
    pub tx_on_loop_iter: Sender<NewLoopIterFnData>,
    pub tx_messages: Sender<WindowManagerMessage>,
}

pub struct Receivers {
    pub rx_components: Receiver<NewCompData>,
    pub rx_on_loop_iter: Receiver<NewLoopIterFnData>,
    pub rx_messages: Receiver<WindowManagerMessage>,
}

/**
API offers an interface between the window-manager and external programs that request
a service from the window-manager, like creating components or subscribing callback-functions to be
executed in the main-loop.
There are three different kinds of positional variables repeatedly mentioned:
- Absolute positions: Denotes positions in relation to the entire screen, used to describe component
positions in their window.
- Relative positions: Denotes positions in relations to their window, as if their window was occupying
the entire screen.
- Logical positions: Denotes positions in relation to their window, as if their window was occupying
the entire screen, with scaling defined by [`LOG_SCREEN`], by applications to describe
their position screen-agnostically.
*/
pub struct Api {
    /// handles are equal to the thread-id of the application
    handles: HashMap<usize, HandleData>,
    senders: Senders,

    /// constant ratios (screen / log_screen)
    pub rel_to_log_ratios: (f64, f64),
}

/// All information saved for a single handle
pub struct HandleData {
    workspace_index: usize,
    window_id: usize,

    /// absolute position on the screen
    abs_pos: RectData,
    root_container: ComponentRef,

    /// ratio to the screen size (abs_size/screen_size)
    ratios: (f64, f64),
}

#[derive(Clone, Copy, Debug)]
pub struct WindowData {
    pub workspace_index: usize,
    pub window_id: usize,
}

pub struct NewCompData {
    pub window_data: WindowData,
    pub parent: ComponentRef,
    pub component: ComponentRef,
}

pub struct NewLoopIterFnData {
    pub window_data: WindowData,
    pub fun: Box<dyn Fn() -> bool>,
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
    pub fn new(senders: Senders) -> Self {
        let screen = SCREEN.get().unwrap();
        Self {
            handles: HashMap::new(),
            senders,
            rel_to_log_ratios: (
                f64::from(screen.0) / f64::from(LOG_SCREEN.0),
                f64::from(screen.1) / f64::from(LOG_SCREEN.1),
            ),
        }
    }

    /// Registers a new window in the given workspace and launches the given app.
    pub fn register(
        &mut self,
        workspace_index: usize,
        window_id: usize,
        abs_pos: RectData,
        app_string: &str,
        root_container: ComponentRef,
    ) -> Result<(), &str> {
        let screen = SCREEN.get().unwrap();
        let app_fn_ptr = self
            .map_app_string_to_fn(app_string)
            .ok_or("App not found")?;

        let handle = thread::create(app_fn_ptr)
            .ok_or("Failed to create thread")?
            .id();
        let handle_data = HandleData {
            workspace_index,
            window_id,
            abs_pos,
            root_container,
            ratios: (
                f64::from(abs_pos.width) / f64::from(screen.0),
                f64::from(abs_pos.height) / f64::from(screen.1),
            ),
        };

        self.handles.insert(handle, handle_data);

        Ok(())
    }

    /// Logical positions need to be contrained by `x <= 1000 && y <= 750`
    pub fn execute(
        &self,
        window_handle: usize,
        parent: Option<ComponentRef>,
        command: Command,
    ) -> Result<ComponentRef, &str> {
        let handle_data = self
            .handles
            .get(&window_handle)
            .ok_or("Provided handle not found")?;

        let window_data = WindowData {
            workspace_index: handle_data.workspace_index,
            window_id: handle_data.window_id,
        };

        let ratios = handle_data.ratios;
        let parent = parent.unwrap_or(handle_data.root_container.clone());

        let component = match command {
            Command::CreateButton {
                log_rect_data,
                label,
                on_click,
                styling,
            } => {
                self.validate_log_pos(&log_rect_data.top_left)?;

                let (text, font_size_option) = label.unzip();
                let font_size = font_size_option.unwrap_or(1);

                let rel_rect_data = self.scale_rect_data_to_rel(&log_rect_data);

                let button = Button::new(
                    rel_rect_data,
                    log_rect_data.clone(),
                    text,
                    font_size,
                    on_click,
                    styling,
                );

                let dispatch_data = NewCompData {
                    window_data,
                    parent,
                    component: Rc::clone(&button),
                };

                self.add_component(dispatch_data);

                button
            }
            Command::CreateLabel {
                log_pos,
                text,
                on_loop_iter,
                font_size,
                styling,
            } => {
                self.validate_log_pos(&log_pos)?;
                let rel_pos = self.scale_vertex_to_rel(&log_pos);

                let font_size = font_size.unwrap_or(1);

                let component = Label::new(rel_pos, font_size, text, styling);

                let dispatch_data = NewCompData {
                    window_data,
                    parent,
                    component: Rc::clone(&component),
                };

                self.add_component(dispatch_data);

                if let Some(fun) = on_loop_iter {
                    let data = NewLoopIterFnData { window_data, fun };
                    self.add_on_loop_iter_fn(data);
                }

                component
            }
            Command::CreateInputField {
                log_rect_data,
                // log_pos,
                width_in_chars,
                font_size,
                starting_text,
                on_change,
                styling,
            } => {
                self.validate_log_pos(&log_rect_data.top_left)?;

                let font_size = font_size.unwrap_or(1);
                let rel_rect_data = self.scale_rect_data_to_rel(&log_rect_data);

                let component = InputField::new(
                    rel_rect_data,
                    font_size,
                    width_in_chars,
                    starting_text,
                    on_change,
                    styling,
                );

                let dispatch_data = NewCompData {
                    window_data,
                    parent,
                    component: Rc::clone(&component),
                };

                self.add_component(dispatch_data);

                component
            }
            Command::CreateCheckbox {
                log_rect_data,
                state,
                on_change,
                styling,
            } => {
                self.validate_log_pos(&log_rect_data.top_left)?;

                let rel_rect_data = self.scale_rect_data_to_rel(&log_rect_data);

                let component = Checkbox::new(
                    rel_rect_data,
                    log_rect_data.clone(),
                    state,
                    on_change,
                    styling,
                );

                let dispatch_data = NewCompData {
                    window_data,
                    parent,
                    component: Rc::clone(&component),
                };

                self.add_component(dispatch_data);

                component
            }
            Command::CreateBitmapGraphic {
                log_rect_data,
                bitmap,
                scaling_mode,
                styling,
            } => {
                self.validate_log_pos(&log_rect_data.top_left)?;

                let rel_rect_data = self.scale_rect_data_to_rel(&log_rect_data);

                let bitmap_graphic = BitmapGraphic::new(
                    rel_rect_data,
                    log_rect_data.clone(),
                    bitmap.clone(),
                    scaling_mode,
                    styling,
                );

                let component = ComponentRef::from_component(Box::new(bitmap_graphic));

                let dispatch_data = NewCompData {
                    window_data,
                    parent,
                    component: Rc::clone(&component),
                };

                self.add_component(dispatch_data);
                Rc::clone(&component)
            }
            Command::CreateSlider {
                log_rect_data,
                on_change,
                value,
                min,
                max,
                steps,
                styling,
            } => {
                self.validate_log_pos(&log_rect_data.top_left)?;

                let rel_rect_data = self.scale_rect_data_to_rel(&log_rect_data);

                let component = Slider::new(
                    rel_rect_data,
                    log_rect_data.clone(),
                    on_change,
                    value,
                    min,
                    max,
                    steps,
                    styling,
                );

                let dispatch_data = NewCompData {
                    window_data,
                    parent,
                    component: Rc::clone(&component),
                };

                self.add_component(dispatch_data);
                Rc::clone(&component)
            }
            Command::CreateRadioButtonGroup {
                center,
                radius,
                spacing,
                num_buttons,
                selected_option,
                on_change,
                styling,
            } => {
                self.validate_log_pos(&center)?;
                let rel_pos = self.scale_vertex_to_rel(&center);
                let rel_radius = self.scale_radius_to_rel(radius);

                let radio_buttons = RadioButtonGroup::new(
                    num_buttons,
                    rel_pos,
                    rel_radius,
                    spacing,
                    selected_option,
                    on_change,
                    styling,
                );

                let component = ComponentRef::from_component(Box::new(radio_buttons));

                let dispatch_data = NewCompData {
                    window_data,
                    parent,
                    component: Rc::clone(&component),
                };

                self.add_component(dispatch_data);

                        component
                    }
            Command::CreateContainer { log_rect_data, layout, styling } => {
                self.validate_log_pos(&log_rect_data.top_left)?;

                let rel_rect_data = self.scale_rect_data_to_rel(&log_rect_data);

                let container = BasicContainer::new(rel_rect_data, layout, styling);
                let component: ComponentRef = ComponentRef::from_component(Box::new(container));

                let dispatch_data = NewCompData {
                    window_data,
                    parent,
                    component: Rc::clone(&component),
                };

                self.add_component(dispatch_data);

                component
            }
            // Julius Drodofsky
            Command::CreateCanvas {
                styling,
                log_rect_data,
                buffer,
                input,
                scaling_mode,
            } => {
                self.validate_log_pos(&log_rect_data.top_left)?;
                let rel_rect_data = self.scale_rect_data_to_rel(&log_rect_data);
                let canvas = Canvas::new(
                    styling,
                    log_rect_data,
                    rel_rect_data,
                    buffer,
                    scaling_mode,
                    input,
                );
                let component = ComponentRef::from_component(Box::new(canvas));
                let dispatch_data = NewCompData {
                    window_data,
                    parent,
                    component: Rc::clone(&component),
                };
                self.add_component(dispatch_data);
                component
            }
        };

        Ok(component)
    }

    /// Sends a message to the window manager to perform various actions.
    /// Window messages will be handled at the end of the current frame.
    pub fn send_message(&self, message: WindowManagerMessage) {
        self.senders
            .tx_messages
            .enqueue(message)
            .expect("message queue was closed!");
    }

    pub fn remove_all_handles_tied_to_workspace(&mut self, workspace_index: usize) {
        self.handles
            .retain(|_, handle| handle.workspace_index != workspace_index);

        self.handles
            .values_mut()
            .filter(|handle| handle.workspace_index > workspace_index)
            .for_each(|handle| handle.workspace_index -= 1);
    }

    pub fn is_app_name_valid(&self, app_string: &str) -> bool {
        self.map_app_string_to_fn(app_string).is_some()
    }

    fn map_app_string_to_fn(&self, app_string: &str) -> Option<fn()> {
        match app_string {
            "clock" => Some(Clock::run),
            "submit_label" => Some(SubmitLabel::run),
            "counter" => Some(Counter::run),
            "slider" => Some(SliderApp::run),
            "bitmap" => Some(BitmapApp::run),
            "calculator" => Some(Calculator::run),
            "radio" => Some(RadioButtonApp::run),
            "canvas" => Some(CanvasApp::run),
            "editor" => Some(TextEditor::run),
            "layout" => Some(LayoutApp::run),
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

    /// Scales a logical rect to a relative rect
    fn scale_rect_data_to_rel(&self, log_rect_data: &RectData) -> RectData {
        let new_pos = Vertex::new(
            (f64::from(log_rect_data.top_left.x) * self.rel_to_log_ratios.0) as u32,
            (f64::from(log_rect_data.top_left.y) * self.rel_to_log_ratios.1) as u32,
        );

        //let aspect_ratio = f64::from(log_rect_data.width) / f64::from(log_rect_data.height);
        //let aspect_ratio = if aspect_ratio == 0.0 || aspect_ratio.is_infinite() { 1.0 } else { aspect_ratio };

        let new_width = f64::from(log_rect_data.width) * self.rel_to_log_ratios.0;
        let new_height = f64::from(log_rect_data.height) * self.rel_to_log_ratios.1;

        return RectData {
            top_left: new_pos,
            width: new_width as u32,
            height: new_height as u32,
        };
    }

    /// Scales a logical vertex to a relative vertex
    fn scale_vertex_to_rel(&self, log_pos: &Vertex) -> Vertex {
        return Vertex::new(
            (f64::from(log_pos.x) * self.rel_to_log_ratios.0) as u32,
            (f64::from(log_pos.y) * self.rel_to_log_ratios.1) as u32,
        );
    }

    /// Scales a logical radius to a relative radius
    fn scale_radius_to_rel(&self, radius: u32) -> u32 {
        return (f64::from(radius) * self.rel_to_log_ratios.0.min(self.rel_to_log_ratios.1)) as u32;
    }

    fn validate_log_pos(&self, log_pos: &Vertex) -> Result<(), &str> {
        if log_pos.x > LOG_SCREEN.0 || log_pos.y > LOG_SCREEN.1 {
            return Err("Logical position-coordinates don't meet size-constraints");
        }

        return Ok(());
    }
}
