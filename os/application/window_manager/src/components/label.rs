use core::ops::Deref;

use alloc::{boxed::Box, rc::Rc, string::String, sync::Arc, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{color::Color, lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH}};
use spin::{Mutex, RwLock};

use crate::{
    observer::{Observable, Observer}, utils::{scale_font, scale_pos_to_window}, SCREEN
};

use super::component::Component;

pub struct Label {
    pub abs_pos: Vertex,
    pub rel_pos: Vertex,
    rel_font_size: usize,
    pub text: Rc<RwLock<String>>,
    pub font_scale: (u32, u32),
    state_dependencies: Vec<Rc<RwLock<Box<dyn Component>>>>,
}

impl Label {
    pub fn new(
        abs_pos: Vertex,
        rel_pos: Vertex,
        rel_font_size: usize,
        text: Rc<RwLock<String>>,
        font_scale: (u32, u32),
        state_dependencies: Vec<Rc<RwLock<Box<dyn Component>>>>,
    ) -> Self {
        Self {
            abs_pos,
            rel_pos,
            rel_font_size,
            text,
            font_scale,
            state_dependencies,
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
        self.font_scale = scale_font(&self.font_scale, &old_window, &new_window);
    }

    fn rescale_after_move(&mut self, new_rect_data: RectData) {
        self.abs_pos = scale_pos_to_window(self.rel_pos, new_rect_data);
        let screen = SCREEN.get().unwrap();
        self.font_scale = scale_font(
            &(self.rel_font_size as u32, self.rel_font_size as u32),
            &RectData {
                top_left: Vertex::new(0, 0),
                width: screen.0,
                height: screen.1,
            },
            &new_rect_data,
        );
    }

    fn get_abs_rect_data(&self) -> RectData {
        RectData {
            top_left: self.abs_pos,
            width: self.text.read().len() as u32 * DEFAULT_CHAR_WIDTH * self.font_scale.0,
            height: DEFAULT_CHAR_HEIGHT * self.font_scale.1,
        }
    }

    fn get_state_dependencies(&self) -> Vec<Rc<RwLock<Box<dyn Component>>>> {
        self.state_dependencies.clone()
    }
}
