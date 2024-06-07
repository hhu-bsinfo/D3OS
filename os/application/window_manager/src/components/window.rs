use alloc::{boxed::Box, vec::Vec};
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::color::{Color, WHITE, YELLOW};
use hashbrown::HashMap;

use crate::{components::component::Component, WindowManager};

use super::{component::Interaction, selected_window_label::SelectedWorkspaceLabel};

pub struct Window {
    pub id: usize,
    pub rect_data: RectData,
    workspace_index: usize,
    components: HashMap<usize, Box<dyn Component>>,
    // focusable components are stored additionally in ordered fashion in here
    component_orderer: Vec<usize>,
    focused_component_id: Option<usize>,
}

impl Window {
    pub fn new(id: usize, workspace_index: usize, rect_data: RectData) -> Self {
        Self {
            id,
            workspace_index,
            components: HashMap::new(),
            component_orderer: Vec::new(),
            rect_data,
            focused_component_id: None,
        }
    }

    pub fn insert_component(&mut self, new_component: Box<dyn Component>, is_focusable: bool) {
        let id = WindowManager::generate_id();
        self.components.insert(id, new_component);

        if is_focusable {
            self.component_orderer.push(id);

            // Focus new (focusable) component, if it is the first one in the window
            if self.component_orderer.len() == 1 {
                self.focused_component_id = Some(id);
            }
        }
    }

    pub fn interact_with_focused_component(&self, interaction: Interaction) {
        if let Some(focused_component_id) = &self.focused_component_id {
            let focused_component = self.components.get(focused_component_id).unwrap();
            focused_component.interact(interaction);
        }
    }

    // LOW_PRIO_TODO: Find a better way to access this singleton of a label in its singleton of a window
    /**
    Draws the number-labels that indicate how many workspaces there are and which
    you are currently on
    */
    pub fn draw_selected_workspace_labels(&self, current_workspace: usize) {
        let filtered_iter = self
            .components
            .values()
            .filter(|comp| comp.as_any().is::<SelectedWorkspaceLabel>());

        for label in filtered_iter {
            let label = label
                .as_any()
                .downcast_ref::<SelectedWorkspaceLabel>()
                .unwrap();
            let color = if current_workspace == label.tied_workspace {
                YELLOW
            } else {
                WHITE
            };

            label.draw(color);
        }
    }

    pub fn focus_next_component(&mut self) {
        if let Some(focused_component_id) = self.focused_component_id {
            let index = self
                .component_orderer
                .iter()
                .position(|comp| *comp == focused_component_id)
                .unwrap();

            let next_index = (index + 1) % self.component_orderer.len();

            self.focused_component_id = Some(self.component_orderer[next_index]);
        }
    }

    pub fn focus_prev_component(&mut self) {
        if let Some(focused_component_id) = self.focused_component_id {
            let index = self
                .component_orderer
                .iter()
                .position(|comp| *comp == focused_component_id)
                .unwrap();

            let prev_index = if index == 0 {
                self.component_orderer.len() - 1
            } else {
                index - 1
            };

            self.focused_component_id = Some(self.component_orderer[prev_index]);
        }
    }

    pub fn draw(&self, color: Color, focused_window_id: usize) {
        let RectData {
            top_left,
            width,
            height,
        } = self.rect_data;
        Drawer::draw_rectangle(
            Vertex::new(top_left.x, top_left.y),
            Vertex::new(top_left.x + width, top_left.y + height),
            color,
        );

        for component in self.components.values() {
            component.draw(WHITE);
        }

        if self.id == focused_window_id {
            if let Some(focused_component_id) = self.focused_component_id {
                self.components
                    .get(&focused_component_id)
                    .unwrap()
                    .draw(YELLOW);
            }
        }
    }
}
