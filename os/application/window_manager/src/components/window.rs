use core::any::Any;
use drawer::drawer::{Drawer, Vertex};
use graphic::color::Color;

use crate::components::component::Component;

pub struct Window {
    pub id: usize,
    pub workspace_index: usize,
    pub pos: Vertex,
    pub width: u32,
    pub height: u32,
}

impl Window {
    pub fn new(id: usize, workspace_index: usize, pos: Vertex, width: u32, height: u32) -> Self {
        Self {
            id,
            workspace_index,
            pos,
            width,
            height,
        }
    }
}

impl Component for Window {
    fn id(&self) -> usize {
        self.id
    }

    fn workspace_index(&self) -> usize {
        self.workspace_index
    }

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
