use alloc::{boxed::Box, collections::LinkedList};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::color::{Color, WHITE};
use hashbrown::HashMap;

use crate::{
    components::component::Component,
    config::{FOCUSED_BG_COLOR, FOCUSED_FG_COLOR},
    utils::get_element_cursor_from_orderer,
    WindowManager,
};

pub const FOCUSED_INDICATOR_COLOR: Color = FOCUSED_BG_COLOR;
pub const FOCUSED_INDICATOR_LENGTH: u32 = 24;

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
    component_orderer: LinkedList<usize>,
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
            component_orderer: LinkedList::new(),
            rect_data,
            focused_component_id: None,
            buddy_window_id: buddy_window,
        }
    }

    pub fn insert_component(&mut self, new_component: Box<dyn Component>, is_focusable: bool) {
        let id = WindowManager::generate_id();
        self.components.insert(id, new_component);

        if is_focusable {
            self.component_orderer.push_back(id);

            // Focus new (focusable) component, if it is the first one in the window
            if self.component_orderer.len() == 1 {
                self.focused_component_id = Some(id);
            }
        }

        self.is_dirty = true;
    }

    pub fn interact_with_focused_component(&mut self, keyboard_press: char) -> bool {
        if let Some(focused_component_id) = &self.focused_component_id {
            let focused_component = self.components.get_mut(focused_component_id).unwrap();
            let did_interact = focused_component.consume_keyboard_press(keyboard_press);
            if did_interact {
                self.is_dirty = true;
                return true;
            }
        }

        return false;
    }

    pub fn focus_next_component(&mut self) {
        if let Some(focused_component_id) = self.focused_component_id {
            let mut cursor =
                get_element_cursor_from_orderer(&mut self.component_orderer, focused_component_id)
                    .unwrap();
            cursor.move_next();

            self.focused_component_id = match cursor.current() {
                Some(next_focused_el) => Some(next_focused_el.clone()),
                None => Some(cursor.peek_next().unwrap().clone()),
            };

            self.is_dirty = true;
        }
    }

    pub fn focus_prev_component(&mut self) {
        if let Some(focused_component_id) = self.focused_component_id {
            let mut cursor =
                get_element_cursor_from_orderer(&mut self.component_orderer, focused_component_id)
                    .unwrap();
            cursor.move_prev();

            self.focused_component_id = match cursor.current() {
                Some(next_focused_el) => Some(next_focused_el.clone()),
                None => Some(cursor.peek_prev().unwrap().clone()),
            };

            self.is_dirty = true;
        }
    }

    pub fn rescale_window_in_place(&mut self, old_rect_data: RectData, new_rect_data: RectData) {
        for component in self.components.values_mut() {
            component.rescale_after_split(old_rect_data, new_rect_data);
        }
    }

    pub fn rescale_window_after_move(&mut self, new_rect_data: RectData) {
        self.rect_data = new_rect_data;

        for component in self.components.values_mut() {
            component.rescale_after_move(new_rect_data);
        }
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

        self.rescale_window_in_place(old_rect, self.rect_data)
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

            Drawer::draw_rectangle(self.rect_data, WHITE);
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
