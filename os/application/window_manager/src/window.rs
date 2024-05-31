use core::any::Any;
use drawer::drawer::{Drawer, Vertex};
use graphic::color::Color;

use crate::components::component::Component;

#[derive(Debug)]
pub(crate) struct Window {
    pub(crate) id: usize,
    pub(crate) pos: Vertex,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Window {
    pub(crate) fn new(id: usize, pos: Vertex, width: u32, height: u32) -> Self {
        Self {
            id,
            pos,
            width,
            height,
        }
    }
}

impl Component for Window {
    fn draw(&self, color: Color) {
        Drawer::draw_rectangle(
            Vertex::new(self.pos.x, self.pos.y),
            Vertex::new(self.pos.x + self.width, self.pos.y + self.height),
            color,
        );
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
