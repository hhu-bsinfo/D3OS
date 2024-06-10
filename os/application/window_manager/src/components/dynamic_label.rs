use core::{any::Any, ops::Deref};

use alloc::{rc::Rc, string::String};
use drawer::drawer::{Drawer, Vertex};
use graphic::color::Color;
use spin::RwLock;

use super::component::{Component, Interaction};

/// Dynamic Labels are characterized by their text being modifiable, unlike [`Label`](`crate::components::label::Label`)
pub struct DynamicLabel {
    pub workspace_index: usize,
    pub pos: Vertex,
    pub text: Rc<RwLock<String>>,
}

impl DynamicLabel {
    pub fn new(workspace_index: usize, pos: Vertex, text: Rc<RwLock<String>>) -> Self {
        Self {
            workspace_index,
            pos,
            text,
        }
    }
}

impl Component for DynamicLabel {
    fn draw(&self, color: Color) {
        let text = self.text.read();
        Drawer::draw_string(
            text.deref().clone(),
            self.pos,
            color,
            (self.font_size, self.font_size),
        );
    }

    fn interact(&self, _interaction: Interaction) {}
}
