use alloc::string::{String, ToString};
use core::any::Any;
use drawer::drawer::{Drawer, Vertex};
use graphic::color::Color;

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
    fn draw(&self, color: Color) {
        Drawer::draw_string(self.text.to_string(), self.pos, color);
    }

    fn interact(&self, _interaction: Interaction) {}
}
