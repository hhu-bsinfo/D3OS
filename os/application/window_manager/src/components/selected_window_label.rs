use alloc::string::{String, ToString};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::color::Color;

use crate::configs::workspace_selection_labels_window::WORKSPACE_SELECTION_LABEL_FONT_SCALE;

use super::component::{Component, Interaction};

pub struct SelectedWorkspaceLabel {
    pub workspace_index: usize,
    pub pos: Vertex,
    pub text: String,
    pub tied_workspace: usize,
}

impl SelectedWorkspaceLabel {
    pub fn new(workspace_index: usize, pos: Vertex, text: String, tied_workspace: usize) -> Self {
        Self {
            workspace_index,
            pos,
            text,
            tied_workspace,
        }
    }
}

impl Component for SelectedWorkspaceLabel {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        Drawer::draw_string(
            self.text.to_string(),
            self.pos,
            fg_color,
            bg_color,
            WORKSPACE_SELECTION_LABEL_FONT_SCALE,
        );
    }

    fn interact(&self, _interaction: Interaction) {}

    fn rescale_in_place(&mut self, _old_window: RectData, _new_window: RectData) {
        // Should never be rescaled
    }

    fn rescale_after_move(&mut self, _new_window_rect_data: RectData) {
        // Should never be moved
    }
}
