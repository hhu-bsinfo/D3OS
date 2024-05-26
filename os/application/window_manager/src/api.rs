use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::color::WHITE;
use hashbrown::HashMap;

extern crate alloc;

pub(crate) enum Command {
    DrawRectangle { top_left: Vertex, width: u32, height: u32 },
    // DrawPolygon,
    // DrawCircle,
}

pub(crate) struct Api {
    pub(crate) handles: HashMap<usize, HandleData>,
    screen_dims: (u32, u32),
}

pub(crate) struct HandleData {
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

    pub(crate) fn register(&mut self, abs_pos: RectData) -> usize {
        let handle = self.handles.len() + 1;
        let handle_data = HandleData {
            abs_pos,
            ratios: (abs_pos.width / self.screen_dims.0, abs_pos.height / self.screen_dims.1),
        };

        self.handles.insert(handle, handle_data);

        return handle;
    }

    pub fn draw(&self, handle: usize, command: Command) -> Result<(), &str> {
        let HandleData { abs_pos, ratios } = self.handles.get(&handle).ok_or("Provided handle not found")?;

        match command {
            Command::DrawRectangle { top_left, width, height } => {
                let draw_top_left = Vertex::new(top_left.x * ratios.0 + abs_pos.top_left.x, top_left.y * ratios.1 + abs_pos.top_left.y);
                let draw_width = width * ratios.0;
                let draw_height = height * ratios.1;

                Drawer::draw_rectangle(
                    draw_top_left, 
                    draw_top_left + Vertex::new(draw_width, draw_height), 
                    WHITE,
                );
            },
        }

        Ok(())
    }
}