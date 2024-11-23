use core::ops::Sub;

use alloc::{string::ToString, vec::{self, Vec}};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::lfb::DEFAULT_CHAR_WIDTH;

use crate::{
    components::{
        component::Component,
        selected_window_label::{
            SelectedWorkspaceLabel, FG_COLOR, UNFOCUSED_BG_COLOR,
            WORKSPACE_SELECTION_LABEL_FONT_SCALE,
        },
    },
    config::{DEFAULT_FG_COLOR, DIST_TO_SCREEN_EDGE, FOCUSED_BG_COLOR},
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

    pub fn insert_label(&mut self, old_workspace_len: usize) {
        let workspace_selection_label = SelectedWorkspaceLabel::new(
            Vertex::new(
                DIST_TO_SCREEN_EDGE
                    + 1
                    + old_workspace_len as u32
                        * DEFAULT_CHAR_WIDTH
                        * WORKSPACE_SELECTION_LABEL_FONT_SCALE.0,
                DIST_TO_SCREEN_EDGE + 1,
            ),
            char::from_digit(old_workspace_len as u32 + 1, 10)
                .unwrap()
                .to_string(),
            old_workspace_len,
            Vec::new(),
        );

        self.labels.push(workspace_selection_label);
    }

    pub fn remove_label(&mut self, tied_workspace: usize) {
        let index = self
            .labels
            .iter_mut()
            .position(|label| label.tied_workspace == tied_workspace)
            .unwrap();

        self.labels.remove(index);

        for label in &mut self.labels[index..] {
            label.pos.x -= DEFAULT_CHAR_WIDTH * WORKSPACE_SELECTION_LABEL_FONT_SCALE.0;
            label.text = label.text.parse::<usize>().unwrap().sub(1).to_string();
            label.tied_workspace -= 1;
        }
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
