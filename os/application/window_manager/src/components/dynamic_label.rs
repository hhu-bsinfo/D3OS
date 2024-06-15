use core::ops::Deref;

use alloc::{rc::Rc, string::String};
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::color::Color;
use spin::RwLock;

use crate::configs::general::DEFAULT_FONT_SCALE;

use super::component::{Component, Interaction};

/// Dynamic Labels are characterized by their text being modifiable, unlike [`Label`](`crate::components::label::Label`)
pub struct DynamicLabel {
    pub workspace_index: usize,
    pub pos: Vertex,
    pub text: Rc<RwLock<String>>,
    pub font_scale: (u32, u32),
}

impl DynamicLabel {
    pub fn new(
        workspace_index: usize,
        pos: Vertex,
        text: Rc<RwLock<String>>,
        font_scale: Option<(u32, u32)>,
    ) -> Self {
        Self {
            workspace_index,
            pos,
            text,
            font_scale: font_scale.unwrap_or(DEFAULT_FONT_SCALE),
        }
    }
}

impl Component for DynamicLabel {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        let text = self.text.read();
        Drawer::draw_string(
            text.deref().clone(),
            self.pos,
            fg_color,
            bg_color,
            self.font_scale,
        );
    }

    fn interact(&self, _interaction: Interaction) {}

    fn rescale(&mut self, old_window: &RectData, new_window: &RectData) {
        self.pos.scale_by_rect_ratio(old_window, new_window);
    }
}
