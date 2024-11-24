use alloc::{boxed::Box, collections::LinkedList, format, rc::Rc};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::color::Color;
use hashbrown::HashMap;
use io::write::log_debug;
use spin::RwLock;

use crate::{
    components::component::Component, config::{DEFAULT_FG_COLOR, FOCUSED_BG_COLOR, FOCUSED_FG_COLOR}, dirty_region::{DirtyRegion, DirtyRegionList}, utils::get_element_cursor_from_orderer, WindowManager
};

pub const FOCUSED_INDICATOR_COLOR: Color = FOCUSED_BG_COLOR;
pub const FOCUSED_INDICATOR_LENGTH: u32 = 24;

/// This is the window used in workspaces to contains components from different apps
pub struct AppWindow {
    pub id: usize,
    pub rect_data: RectData,
    /// Indicates whether redrawing of this window is required in next loop-iteration
    pub is_dirty: bool,
    pub dirty_regions: DirtyRegionList,
    components: HashMap<usize, Rc<RwLock<Box<dyn Component>>>>,
    /// focusable components are stored additionally in ordered fashion in here
    component_orderer: LinkedList<usize>,
    focused_component_id: Option<usize>,
}

impl AppWindow {
    pub fn new(id: usize, rect_data: RectData) -> Self {
        Self {
            id,
            is_dirty: true,
            dirty_regions: DirtyRegionList::new(),
            components: HashMap::new(),
            component_orderer: LinkedList::new(),
            rect_data,
            focused_component_id: None,
        }
    }

    pub fn mark_dirty_region(&mut self, dirty_region: DirtyRegion) {
        self.dirty_regions.add(dirty_region);
    }

    pub fn mark_component_dirty(&mut self, component: &Rc<RwLock<Box<dyn Component>>>) {
        let dirty_region = DirtyRegion::new(component.read().get_abs_rect_data());

        for depend_component in component.read().get_redraw_components() {
            self.mark_component_dirty(&depend_component);
        }
        
        self.mark_dirty_region(dirty_region);
    }

    pub fn mark_window_dirty(&mut self) {
        self.is_dirty = true;
    }

    fn mark_window_region_dirty(&mut self) {
        self.mark_dirty_region(DirtyRegion::new(self.rect_data.sub_border()));
    }

    pub fn insert_component(&mut self, new_component: Rc<RwLock<Box<dyn Component>>>, is_focusable: bool) {
        let id = WindowManager::generate_id();
        
        if is_focusable {
            self.component_orderer.push_back(id);
            
            // Focus new (focusable) component, if it is the first one in the window
            if self.component_orderer.len() == 1 {
                self.focused_component_id = Some(id);
            }
        }
        
        
        // region of new component is dirty
        self.mark_component_dirty(&new_component);
        self.components.insert(id, new_component);
    }

    pub fn interact_with_focused_component(&mut self, keyboard_press: char) -> bool {
        if let Some(focused_component_id) = &self.focused_component_id {
            let focused_component = Rc::clone(self.components.get(focused_component_id).unwrap());
            let did_interact = focused_component.write().consume_keyboard_press(keyboard_press);

            if did_interact {
                // region of focused component is dirty
                self.mark_component_dirty(&focused_component);
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

            let old_focused_component = Rc::clone(self.components.get(&focused_component_id).unwrap());

            self.focused_component_id = match cursor.current() {
                Some(next_focused_el) => Some(next_focused_el.clone()),
                None => Some(cursor.peek_next().unwrap().clone()),
            };

            let next_focused_component_id = self.focused_component_id.unwrap();
            let next_focused_component = Rc::clone(self.components.get(&next_focused_component_id).unwrap());

            self.mark_component_dirty(&old_focused_component);
            self.mark_component_dirty(&next_focused_component);
        }
    }

    pub fn focus_prev_component(&mut self) {
        if let Some(focused_component_id) = self.focused_component_id {
            let mut cursor =
                get_element_cursor_from_orderer(&mut self.component_orderer, focused_component_id)
                    .unwrap();
            cursor.move_prev();

            let old_focused_component_id = focused_component_id.clone();
            let old_focused_component = Rc::clone(self.components.get(&focused_component_id).unwrap());

            self.focused_component_id = match cursor.current() {
                Some(next_focused_el) => Some(next_focused_el.clone()),
                None => Some(cursor.peek_prev().unwrap().clone()),
            };

            let next_focused_component_id = self.focused_component_id.unwrap();
            let next_focused_component = Rc::clone(self.components.get(&next_focused_component_id).unwrap());

            // region of both components is dirty
            self.mark_component_dirty(&old_focused_component);
            self.mark_component_dirty(&next_focused_component);
        }
    }

    pub fn rescale_window_in_place(&mut self, old_rect_data: RectData, new_rect_data: RectData) {
        let components = self.components.values();
        for component in components {
            // self.mark_component_dirty(&component);
            component.write().rescale_after_split(old_rect_data, new_rect_data);
        }

        self.mark_window_dirty();
    }

    pub fn rescale_window_after_move(&mut self, new_rect_data: RectData) {
        self.rect_data = new_rect_data;

        for component in self.components.values_mut() {
            component.write().rescale_after_move(new_rect_data);
        }
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
            Drawer::draw_rectangle(self.rect_data, DEFAULT_FG_COLOR);
            self.is_dirty = true;
        }

        if self.is_dirty {
            self.mark_window_region_dirty();
        }
        
        // Nothing to redraw
        if self.dirty_regions.is_empty() {
            return;
        } 

        let is_focused = self.id == focused_window_id;

        for region in self.dirty_regions.regions.iter() {
            Drawer::partial_clear_screen(region.rect);

            for component in self.components.values() {
                // only redraw components in the dirty region
                if region.rect.intersects(&component.read().get_abs_rect_data()) {
                    component.read().draw(DEFAULT_FG_COLOR, None);
                }
            }
        }

        if is_focused {
            self.draw_is_focused_indicator();

            if let Some(focused_component_id) = self.focused_component_id {
                self.components
                    .get(&focused_component_id)
                    .unwrap()
                    .read()
                    .draw(FOCUSED_FG_COLOR, None);
            }
        }

        // dirty regions redrawn
        self.dirty_regions.clear();
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
