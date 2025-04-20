use core::ops::Sub;

use alloc::{string::ToString, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{color::{Color, WHITE}, lfb::DEFAULT_CHAR_WIDTH};

pub const TEXT_COLOR: Color = WHITE;

pub const LABEL_BG_COLOR_FOCUSED: Color = FOCUSED_BG_COLOR;
pub const LABEL_BG_COLOR_UNFOCUSED: Color = UNFOCUSED_BG_COLOR;

use crate::{
    components::{
        component::{Component, ComponentStylingBuilder},
        selected_window_label::{
            SelectedWorkspaceLabel,
            WORKSPACE_SELECTION_LABEL_FONT_SCALE,
        },
    },
    config::{DEFAULT_FG_COLOR, DIST_TO_SCREEN_EDGE, FOCUSED_BG_COLOR, UNFOCUSED_BG_COLOR}, mouse_state::{ButtonState, MouseEvent},
};

pub struct WorkspaceSelectionLabelsWindow {
    is_dirty: bool,
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

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn insert_label(&mut self, old_workspace_len: usize) {
        let styling = ComponentStylingBuilder::new()
            .background_color(UNFOCUSED_BG_COLOR)
            .focused_background_color(FOCUSED_BG_COLOR)
            .build();
    
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
            Some(styling),
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

        for label in self.labels.iter_mut() {
            let focused = label.tied_workspace == current_workspace;
            label.mark_dirty();
            label.draw(if focused { label.id } else { None });
        }

        self.is_dirty = false;
    }

    pub fn handle_mouse_event(&mut self, mouse_event: &MouseEvent) -> Option<usize> {
        for label in self.labels.iter_mut() {
            if label.get_abs_rect_data().contains_vertex(&mouse_event.position) {
                if mouse_event.buttons.left == ButtonState::Pressed {
                    return Some(label.tied_workspace);
                }
            }
        }

        None
    }
}
