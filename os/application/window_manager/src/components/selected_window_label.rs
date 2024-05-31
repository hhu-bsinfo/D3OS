use alloc::string::{String, ToString};
use core::any::Any;
use drawer::drawer::{Drawer, Vertex};
use graphic::color::Color;

use super::component::Component;

pub struct SelectedWorkspaceLabel {
    pub id: usize,
    pub pos: Vertex,
    pub text: String,
    pub tied_workspace: usize,
}

impl SelectedWorkspaceLabel {
    pub fn new(id: usize, pos: Vertex, text: String, tied_workspace: usize) -> Self {
        Self {
            id,
            pos,
            text,
            tied_workspace,
        }
    }
}

impl Component for SelectedWorkspaceLabel {
    fn id(&self) -> usize {
        self.id
    }

    fn draw(&self, color: Color) {
        Drawer::draw_string(self.text.to_string(), self.pos, color);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
