use alloc::{boxed::Box, vec::Vec};
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::color::WHITE;
use hashbrown::HashMap;

use crate::{
    components::component::{Component, Interaction},
    configs::{
        app_window::{FOCUSED_INDICATOR_COLOR, FOCUSED_INDICATOR_LENGTH},
        general::FOCUSED_FG_COLOR,
    },
    WindowManager,
};

/// This is the window used in workspaces to contains components from different apps
pub struct AppWindow {
    pub id: usize,
    pub rect_data: RectData,
    /// Indicates whether redrawing of this window is required in next loop-iteration
    pub is_dirty: bool,
    /// The buddy of this window, used to decide how closing this window works
    pub buddy_window_id: Option<usize>,
    /// The workspace this window belongs to
    workspace_index: usize,
    components: HashMap<usize, Box<dyn Component>>,
    /// focusable components are stored additionally in ordered fashion in here
    component_orderer: Vec<usize>,
    focused_component_id: Option<usize>,
}

impl AppWindow {
    pub fn new(
        id: usize,
        workspace_index: usize,
        rect_data: RectData,
        buddy_window: Option<usize>,
    ) -> Self {
        Self {
            id,
            is_dirty: true,
            workspace_index,
            components: HashMap::new(),
            component_orderer: Vec::new(),
            rect_data,
            focused_component_id: None,
            buddy_window_id: buddy_window,
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

        self.is_dirty = true;
    }

    pub fn interact_with_focused_component(&mut self, interaction: Interaction) {
        if let Some(focused_component_id) = &self.focused_component_id {
            let focused_component = self.components.get(focused_component_id).unwrap();
            focused_component.interact(interaction);
        }
        self.is_dirty = true;
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

        self.is_dirty = true;
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

        self.is_dirty = true;
    }

    pub fn rescale_components(&mut self, old_window: RectData, new_window: RectData) {
        self.components
            .values_mut()
            .for_each(|component| component.rescale(&old_window, &new_window))
    }

    pub fn is_elligible_for_merging(&self, other_window: &AppWindow) -> bool {
        &self.rect_data.width == &other_window.rect_data.width
            && &self.rect_data.height == &other_window.rect_data.height
    }

    pub fn merge(&mut self, other_window: &AppWindow) {
        let old_rect @ RectData {
            top_left: old_top_left,
            width: old_width,
            height: old_height,
        } = self.rect_data;
        let other_top_left = other_window.rect_data.top_left;
        let mut new_top_left = old_top_left;
        let mut new_width = old_width;
        let mut new_height = old_height;

        // We have a vertical splittype, the € mark the different top_left.x coords
        // €########€#########      ###################
        // #        |        #      #                 #
        // #        |        #      #                 #
        // #        |        # ===> #                 #
        // #        |        #      #                 #
        // #        |        #      #                 #
        // ###################      ###################
        if old_top_left.x != other_top_left.x {
            assert_eq!(old_top_left.y, other_top_left.y);
            new_top_left = Vertex::new(u32::min(old_top_left.x, other_top_left.x), old_top_left.y);
            new_width = old_width * 2;
        }
        // We have a horizontal splittype, the € mark the different top_left.y coords
        // €##################      ###################
        // #                 #      #                 #
        // #                 #      #                 #
        // €-----------------# ===> #                 #
        // #                 #      #                 #
        // #                 #      #                 #
        // ###################      ###################
        else if old_top_left.y != other_top_left.y {
            assert_eq!(old_top_left.x, other_top_left.x);
            new_top_left = Vertex::new(old_top_left.x, u32::min(old_top_left.y, other_top_left.y));
            new_height = old_height * 2;
        }

        self.rect_data = RectData {
            top_left: new_top_left,
            width: new_width,
            height: new_height,
        };

        self.rescale_components(old_rect, self.rect_data)
    }

    pub fn draw(&mut self, focused_window_id: usize, full: bool) {
        if full {
            self.is_dirty = true;
        }

        if !self.is_dirty {
            return;
        }

        let is_focused = self.id == focused_window_id;

        let RectData {
            top_left,
            width,
            height,
        } = self.rect_data;

        if full {
            Drawer::partial_clear_screen(self.rect_data);

            Drawer::draw_rectangle(
                Vertex::new(top_left.x, top_left.y),
                Vertex::new(top_left.x + width, top_left.y + height),
                WHITE,
            );
        } else {
            // Clear everything except the border
            Drawer::partial_clear_screen(RectData {
                top_left: top_left.add(1, 1),
                width: width - 2,
                height: height - 2,
            });
        }

        for component in self.components.values() {
            component.draw(WHITE, None);
        }

        if is_focused {
            self.draw_is_focused_indicator();

            if let Some(focused_component_id) = self.focused_component_id {
                self.components
                    .get(&focused_component_id)
                    .unwrap()
                    .draw(FOCUSED_FG_COLOR, None);
            }
        }

        self.is_dirty = false;
    }

    fn draw_is_focused_indicator(&self) {
        let top_left = self.rect_data.top_left;
        let side_length = FOCUSED_INDICATOR_LENGTH;
        let vertices = [
            top_left.add(1, 1),
            top_left.add(side_length, 1),
            top_left.add(1, side_length),
        ];
        Drawer::draw_filled_triangle(vertices, FOCUSED_INDICATOR_COLOR);
    }
}
