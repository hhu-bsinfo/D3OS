use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use alloc::vec;

use crate::config::DEFAULT_FG_COLOR;

pub struct MouseCursor {
    position: (u32, u32),
    last_position: (u32, u32),
}

impl MouseCursor {
    pub fn new() -> Self {
        Self {
            position: (0, 0),
            last_position: (0, 0),
        }
    }

    pub fn update(&mut self, x: u32, y: u32) {
        self.position = (x, y);
    }

    pub fn draw(&mut self) {
        Drawer::flush_lines(self.last_position.1, 11);
            
        Drawer::draw_polygon_direct(
            vec![
                Vertex::new(self.position.0, self.position.1),
                Vertex::new(self.position.0 + 10, self.position.1 + 4),
                Vertex::new(self.position.0 + 4, self.position.1 + 10),
            ],
            DEFAULT_FG_COLOR,
        );
            
        self.last_position = self.position;
    }
}