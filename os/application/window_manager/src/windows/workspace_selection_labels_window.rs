use crate::configs::{general::FOCUSED_BG_COLOR, workspace_selection_labels_window};
use alloc::vec::Vec;
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::color::WHITE;

use crate::components::{component::Component, selected_window_label::SelectedWorkspaceLabel};

pub struct WorkspaceSelectionLabelsWindow {
    rect_data: RectData,
    labels: Vec<SelectedWorkspaceLabel>,
}

impl WorkspaceSelectionLabelsWindow {
    pub fn new(rect_data: RectData) -> Self {
        Self {
            rect_data,
            labels: Vec::new(),
        }
    }

    pub fn insert_label(&mut self, label: SelectedWorkspaceLabel) {
        self.labels.push(label);
    }

    pub fn draw(&mut self, current_workspace: usize, with_borders: bool) {
        let RectData {
            top_left,
            width,
            height,
        } = self.rect_data;

        if with_borders {
            Drawer::partial_clear_screen(self.rect_data);

            Drawer::draw_rectangle(
                Vertex::new(top_left.x, top_left.y),
                Vertex::new(top_left.x + width, top_left.y + height),
                WHITE,
            );
        } else {
            Drawer::partial_clear_screen(RectData {
                top_left: top_left.add(1, 1),
                width: width - 2,
                height: height - 2,
            });
        }

        for label in self.labels.iter() {
            let bg_color = if label.tied_workspace == current_workspace {
                FOCUSED_BG_COLOR
            } else {
                workspace_selection_labels_window::UNFOCUSED_BG_COLOR
            };

            label.draw(workspace_selection_labels_window::FG_COLOR, Some(bg_color));
        }
    }
}
