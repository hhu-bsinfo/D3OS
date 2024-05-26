use alloc::boxed::Box;
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::color::WHITE;
use hashbrown::HashMap;

use crate::{components::button::Button, WindowManager};

extern crate alloc;

pub enum Command {
    DrawRectangle { pos: RectData },
    CreateButton { 
        pos: RectData,
        label: Option<&'static str>,
        on_click: Box<dyn FnMut() -> ()>
    }
}

pub struct Api {
    pub handles: HashMap<usize, HandleData>,
    screen_dims: (u32, u32),
}

pub struct HandleData {
    workspace_index: usize,
    window_id: usize,
    abs_pos: RectData,
    ratios: (u32, u32),
}

impl Api {
    pub fn new(screen_dims: (u32, u32)) -> Self {
        Self {
            handles: HashMap::new(),
            screen_dims,
        }
    }

    pub fn register(&mut self, workspace_index: usize, window_id: usize, abs_pos: RectData) -> usize {
        let handle = self.handles.len() + 1;
        let handle_data = HandleData {
            workspace_index,
            window_id,
            abs_pos,
            ratios: (abs_pos.width / self.screen_dims.0, abs_pos.height / self.screen_dims.1),
        };

        self.handles.insert(handle, handle_data);

        return handle;
    }

    pub fn execute(&self, handle: usize, command: Command) -> Result<(), &str> {
        let HandleData { workspace_index, window_id, abs_pos, ratios } = 
            self.handles.get(&handle).ok_or("Provided handle not found")?;

        match command {
            Command::DrawRectangle { pos: RectData { top_left, width, height } } => {
                let draw_top_left = Vertex::new(top_left.x * ratios.0 + abs_pos.top_left.x, top_left.y * ratios.1 + abs_pos.top_left.y);
                let draw_width = width * ratios.0;
                let draw_height = height * ratios.1;

                Drawer::draw_rectangle(
                    draw_top_left, 
                    draw_top_left + Vertex::new(draw_width, draw_height), 
                    WHITE,
                );
            },
            Command::CreateButton { pos, label, on_click } => {
                let button = Button::new(
                    WindowManager::generate_id(),
                    *window_id,
                    pos,
                    label.unwrap_or_default(),
                    on_click,
                );
            }
        }

        Ok(())
    }
}