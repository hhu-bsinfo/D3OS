use alloc::string::{String, ToString};
use drawer::drawer::{Drawer, Vertex};
use graphic::color::Color;

use crate::configs::general::DEFAULT_FONT_SCALE;

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
    pub font_scale: (u32, u32),
}

impl Label {
    pub fn new(
        workspace_index: usize,
        pos: Vertex,
        text: String,
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

impl Component for Label {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        Drawer::draw_string(
            self.text.to_string(),
            self.pos,
            fg_color,
            bg_color,
            self.font_scale,
        );
    }

    fn interact(&self, _interaction: Interaction) {}

    fn rescale(
        &mut self,
        _old_window: &drawer::drawer::RectData,
        _new_window: &drawer::drawer::RectData,
        translate_by: (i32, i32),
    ) {
        self.pos.x.saturating_add_signed(translate_by.0);
        self.pos.y.saturating_add_signed(translate_by.1);
    }
}
