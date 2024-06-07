use alloc::{boxed::Box, vec::Vec};
use core::any::Any;
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::color::{Color, WHITE, YELLOW};
use hashbrown::HashMap;

use crate::{components::component::Component, WindowManager};

use super::selected_window_label::SelectedWorkspaceLabel;

pub struct Window {
    pub id: usize,
    pub workspace_index: usize,
    pub components: HashMap<usize, Box<dyn Component>>,
    pub component_orderer: Vec<usize>,
    pub rect_data: RectData,
}

impl Window {
    pub fn new(id: usize, workspace_index: usize, rect_data: RectData) -> Self {
        Self {
            id,
            workspace_index,
            components: HashMap::new(),
            component_orderer: Vec::new(),
            rect_data,
        }
    }

    pub fn insert_component(&mut self, new_component: Box<dyn Component>, is_focusable: bool) {
        let id = WindowManager::generate_id();
        self.components.insert(id, new_component);

        if is_focusable {
            self.component_orderer.push(id);
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
}

impl Component for Window {
    fn id(&self) -> usize {
        self.id
    }

    fn workspace_index(&self) -> usize {
        self.workspace_index
    }

    fn draw(&self, color: Color) {
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
        for (_, component) in self.components.iter() {
            component.draw(WHITE);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
