use alloc::{boxed::Box, string::String};
use concurrent::thread;
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::color::WHITE;
use hashbrown::HashMap;
use nolock::queues::mpsc::jiffy::Sender;

use crate::{
    apps::{runnable::Runnable, test_app::TestApp},
    components::{button::Button, component::Component},
    WindowManager,
};

extern crate alloc;

pub enum Command {
    DrawRectangle {
        pos: RectData,
    },
    CreateButton {
        pos: RectData,
        label: Option<String>,
        on_click: Box<dyn Fn() -> ()>,
    },
}

pub struct Api {
    pub handles: HashMap<usize, HandleData>,
    screen_dims: (u32, u32),
    tx_wm: Sender<Box<dyn Component>>,
}

pub struct HandleData {
    workspace_index: usize,
    window_id: usize,
    abs_pos: RectData,
    ratios: (f64, f64),
}

impl Api {
    pub fn new(screen_dims: (u32, u32), tx_wm: Sender<Box<dyn Component>>) -> Self {
        Self {
            handles: HashMap::new(),
            screen_dims,
            tx_wm,
        }
    }

    pub fn register(
        &mut self,
        workspace_index: usize,
        window_id: usize,
        abs_pos: RectData,
    ) -> usize {
        let handle = thread::create(TestApp::run).id();
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

        return handle;
    }

    pub fn execute(&self, handle: usize, command: Command) -> Result<(), &str> {
        let handle_data = self
            .handles
            .get(&handle)
            .ok_or("Provided handle not found")?;

        match command {
            Command::DrawRectangle { pos } => {
                let RectData {
                    top_left,
                    width,
                    height,
                } = self.scale_to_window(pos, handle_data);

                Drawer::draw_rectangle(top_left, top_left + Vertex::new(width, height), WHITE);
            }
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
                self.add_component(Box::new(button));
            }
        }

        Ok(())
    }

    fn add_component(&self, component: Box<dyn Component>) {
        self.tx_wm.enqueue(component);
    }

    fn scale_to_window(
        &self,
        RectData {
            top_left,
            width,
            height,
        }: RectData,
        HandleData {
            workspace_index: _,
            window_id: _,
            abs_pos,
            ratios,
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
