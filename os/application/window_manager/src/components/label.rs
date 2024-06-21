use alloc::string::{String, ToString};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
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
    pub abs_pos: Vertex,
    pub rel_pos: Vertex,
    pub text: String,
    pub font_scale: (u32, u32),
}

impl Label {
    pub fn new(
        workspace_index: usize,
        abs_pos: Vertex,
        rel_pos: Vertex,
        text: String,
        font_scale: Option<(u32, u32)>,
    ) -> Self {
        Self {
            workspace_index,
            abs_pos,
            rel_pos,
            text,
            font_scale: font_scale.unwrap_or(DEFAULT_FONT_SCALE),
        }
    }
}

impl Component for Label {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        Drawer::draw_string(
            self.text.to_string(),
            self.abs_pos,
            fg_color,
            bg_color,
            self.font_scale,
        );
    }

    fn interact(&self, _interaction: Interaction) {}

    fn rescale_in_place(&mut self, old_window: RectData, new_window: RectData) {
        self.abs_pos = self.abs_pos.scale_by_rect_ratio(&old_window, &new_window);
    }

    fn rescale_after_move(&mut self, new_window_rect_data: RectData) {
        self.abs_pos = new_window_rect_data
            .top_left
            .add(self.rel_pos.x, self.rel_pos.y);
    }
}
