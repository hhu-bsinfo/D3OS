use alloc::boxed::Box;
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::lfb::DEFAULT_CHAR_HEIGHT;
use crate::{config::INTERACT_BUTTON, utils::{scale_pos_to_window, scale_radius_to_window, scale_rect_to_window}};

use super::component::{Casts, Component, ComponentStyling, Interactable};

pub struct RadioButton {
    pub id: Option<usize>,
    
    abs_center: Vertex,
    rel_center: Vertex,
    abs_radius: u32,
    rel_radius: u32,
    drawn_rect_data: RectData,

    pub state: bool,
    
    is_disabled: bool,
    is_hidden: bool,
    is_dirty: bool,

    styling: ComponentStyling,
}

impl RadioButton {
    pub fn new(
        id: usize,
        abs_center: Vertex,
        rel_center: Vertex,
        abs_radius: u32,
        rel_radius: u32,
        state: bool,
        styling: Option<ComponentStyling>,
    ) -> Self {
        let drawn_rect_data = RectData {
            top_left: abs_center.sub(abs_radius, abs_radius),
            width: abs_radius * 2,
            height: abs_radius * 2,
        };

        Self {
            id: Some(id),
            abs_center,
            rel_center,
            abs_radius,
            rel_radius,
            drawn_rect_data,
            state,
            is_disabled: false,
            is_hidden: false,
            is_dirty: true,
            styling: styling.unwrap_or_default(),
        }
    }

    pub fn set_state(&mut self, state: bool) {
        self.state = state;
        self.is_dirty = true;
    }

    pub fn set_radius(&mut self, radius: u32) {
        self.abs_radius = radius;
        self.is_dirty = true;
    }

    pub fn set_center(&mut self, center: Vertex) {
        self.abs_center = center;
        self.is_dirty = true;
    }
}

impl Component for RadioButton {
    fn draw(&mut self, focus_id: Option<usize>) {
        // if !self.is_dirty {
        //     return;
        // }

        if self.is_hidden {
            self.is_dirty = false;
            return;
        }

        let styling = &self.styling;
        let is_focused = focus_id == self.id;

        let bg_color = if self.is_disabled {
            styling.disabled_background_color
        } else if is_focused {
            styling.focused_background_color
        } else {
            styling.background_color
        };

        let border_color = if self.is_disabled {
            styling.disabled_border_color
        } else if is_focused {
            styling.focused_border_color
        } else {
            styling.border_color
        };

        Drawer::draw_circle(self.abs_center, self.abs_radius, border_color);

        self.drawn_rect_data = self.get_abs_rect_data();

        if self.state {
            let inner_rad = (self.abs_radius as f32 * 0.5) as u32;
            Drawer::draw_filled_circle(self.abs_center, inner_rad, border_color, None);
        }

        self.is_dirty = false;
    }

    fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    fn get_id(&self) -> Option<usize> {
        self.id
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
    }

    fn get_abs_rect_data(&self) -> RectData {
        RectData {
            top_left: self.abs_center.sub(self.abs_radius, self.abs_radius),
            width: self.abs_radius * 2,
            height: self.abs_radius * 2,
        }
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }

    fn rescale_after_split(&mut self, old_window: RectData, new_window: RectData) {
       // wird in button_group übernommen
    }

    fn rescale_after_move(&mut self, new_rect_data: RectData) {
        // wird in button_group übernommen
        
        self.mark_dirty();
    }
}

impl Casts for RadioButton {}
