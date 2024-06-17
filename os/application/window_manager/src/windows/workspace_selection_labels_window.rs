use crate::configs::{general::FOCUSED_BG_COLOR, workspace_selection_labels_window};
use alloc::vec::Vec;
use drawer::drawer::{Drawer, RectData};
use graphic::color::WHITE;

use crate::components::{component::Component, selected_window_label::SelectedWorkspaceLabel};

pub struct WorkspaceSelectionLabelsWindow {
    pub is_dirty: bool,
    rect_data: RectData,
    labels: Vec<SelectedWorkspaceLabel>,
}

impl WorkspaceSelectionLabelsWindow {
    pub fn new(rect_data: RectData) -> Self {
        Self {
            rect_data,
            labels: Vec::new(),
            is_dirty: true,
        }
    }

    pub fn insert_label(&mut self, label: SelectedWorkspaceLabel) {
        self.labels.push(label);
    }

    pub fn draw(&mut self, current_workspace: usize, dirty_override: bool) {
        if !self.is_dirty && !dirty_override {
            return;
        }

        Drawer::partial_clear_screen(self.rect_data);
        Drawer::draw_rectangle(self.rect_data, WHITE);

        for label in self.labels.iter() {
            let bg_color = if label.tied_workspace == current_workspace {
                FOCUSED_BG_COLOR
            } else {
                workspace_selection_labels_window::UNFOCUSED_BG_COLOR
            };

            label.draw(workspace_selection_labels_window::FG_COLOR, Some(bg_color));
        }

        self.is_dirty = false;
    }
}
