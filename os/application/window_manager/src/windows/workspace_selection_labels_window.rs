use alloc::vec::Vec;
use drawer::{drawer::Drawer, rect_data::RectData};

use crate::{
    components::{
        component::Component,
        selected_window_label::{SelectedWorkspaceLabel, FG_COLOR, UNFOCUSED_BG_COLOR},
    },
    config::{DEFAULT_FG_COLOR, FOCUSED_BG_COLOR},
};

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
        Drawer::draw_rectangle(self.rect_data, DEFAULT_FG_COLOR);

        for label in self.labels.iter() {
            let bg_color = if label.tied_workspace == current_workspace {
                FOCUSED_BG_COLOR
            } else {
                UNFOCUSED_BG_COLOR
            };

            label.draw(FG_COLOR, Some(bg_color));
        }

        self.is_dirty = false;
    }
}
