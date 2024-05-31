use alloc::string::{String, ToString};
use core::any::Any;
use drawer::drawer::{Drawer, Vertex};
use graphic::color::Color;

use super::component::Component;

/**
Since [`draw_string()`](drawer::drawer::Drawer) uses the 8x8-font impl on kernel-side,
we cannot specify a size to the characters until we implemented font-handling ourselves
(which I reeeaaaaaaally don't want to do)
*/
#[derive(Debug)]
pub(crate) struct Label {
    pub(crate) id: usize,
    pub(crate) pos: Vertex,
    pub(crate) text: String,
}

impl Label {
    pub(crate) fn new(id: usize, pos: Vertex, text: String) -> Self {
        Self { id, pos, text }
    }
}

impl Component for Label {
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
