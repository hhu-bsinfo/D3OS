use core::ops::Deref;

use alloc::{rc::Rc, string::String};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::color::Color;
use spin::RwLock;

use crate::utils::scale_pos_to_window;

use super::component::Component;

pub struct Label {
    pub workspace_index: usize,
    pub abs_pos: Vertex,
    pub rel_pos: Vertex,
    pub text: Rc<RwLock<String>>,
    pub font_scale: (u32, u32),
}

impl Label {
    pub fn new(
        workspace_index: usize,
        abs_pos: Vertex,
        rel_pos: Vertex,
        text: Rc<RwLock<String>>,
        font_scale: (u32, u32),
    ) -> Self {
        Self {
            workspace_index,
            abs_pos,
            rel_pos,
            text,
            font_scale,
        }
    }
}

impl Component for Label {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        let text = self.text.read();
        Drawer::draw_string(
            text.deref().clone(),
            self.abs_pos,
            fg_color,
            bg_color,
            self.font_scale,
        );
    }

    fn consume_keyboard_press(&mut self, _keyboard_press: char) -> bool {
        return false;
    }

    fn rescale_after_split(&mut self, old_window: RectData, new_window: RectData) {
        self.abs_pos = self.abs_pos.move_to_new_rect(&old_window, &new_window);
    }

    fn rescale_after_move(&mut self, new_window_rect_data: RectData) {
        self.abs_pos = scale_pos_to_window(self.rel_pos, new_window_rect_data);
    }
}
