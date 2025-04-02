use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use alloc::{format, vec};
use input::mouse::MousePacket;
use terminal::write::log_debug;

use crate::config::DEFAULT_FG_COLOR;

pub struct MouseState {
    position: (u32, u32),
    last_position: (u32, u32),
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            position: (0, 0),
            last_position: (0, 0),
        }
    }

    pub fn update(&mut self, mouse_packet: &MousePacket) {
        self.position.0 = self.position.0.saturating_add_signed(mouse_packet.dx as i32);
        self.position.1 = self.position.1.saturating_add_signed(-mouse_packet.dy as i32);

        /*log_debug(&format!(
            "Mouse position: x: {}, y: {}",
            self.position.0, self.position.1
        ));*/

        // TODO: Clamp to screen size
    }

    pub fn position(&self) -> (u32, u32) {
        self.position
    }

    pub fn draw_cursor(&mut self) {
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