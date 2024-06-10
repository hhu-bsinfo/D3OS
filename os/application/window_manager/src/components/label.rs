use alloc::string::{String, ToString};
use core::any::Any;
use drawer::drawer::{Drawer, Vertex};
use graphic::color::Color;

use super::component::{Component, Interaction};

/**
Since [`draw_string()`](drawer::drawer::Drawer) uses the 8x8-font impl on kernel-side,
we cannot specify a size to the characters until we implemented font-handling ourselves
(which I reeeaaaaaaally don't want to do)
*/
#[derive(Debug)]
pub struct Label {
    pub workspace_index: usize,
    pub pos: Vertex,
    pub text: String,
    pub font_size: usize,
}

impl Label {
    pub fn new(
        workspace_index: usize,
        pos: Vertex,
        text: String,
        font_size: Option<usize>,
    ) -> Self {
        Self {
            workspace_index,
            pos,
            text,
            font_size: font_size.unwrap_or(1),
        }
    }
}

impl Component for Label {
    fn draw(&self, color: Color) {
        Drawer::draw_string(self.text.to_string(), self.pos, color);
    }

    fn interact(&self, _interaction: Interaction) {}

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
