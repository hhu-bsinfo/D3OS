use alloc::string::String;
use drawer::{drawer::Drawer, rect_data::RectData};
use graphic::color::Color;

use crate::{
    configs::general::{DEFAULT_FONT_SCALE, INTERACT_BUTTON},
    utils::scale_rect_to_window,
};

use super::component::Component;

pub struct InputField {
    /**
    If we are selected, all keyboard input is redirected to us, unless
    command-line-window is opened
    */
    pub is_selected: bool,
    pub workspace_index: usize,
    pub abs_rect_data: RectData,
    pub rel_rect_data: RectData,
    pub current_text: String,
}

impl InputField {
    fn new(workspace_index: usize, abs_rect_data: RectData, rel_rect_data: RectData) -> Self {
        Self {
            is_selected: false,
            workspace_index,
            abs_rect_data,
            rel_rect_data,
            current_text: String::with_capacity(16),
        }
    }
}

impl Component for InputField {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        Drawer::draw_rectangle(self.abs_rect_data, fg_color);
        Drawer::draw_string(
            self.current_text.clone(),
            self.abs_rect_data.top_left.add(2, 2),
            fg_color,
            bg_color,
            DEFAULT_FONT_SCALE,
        );
    }

    fn consume_keyboard_press(&mut self, keyboard_press: char) -> bool {
        if keyboard_press == INTERACT_BUTTON && !self.is_selected {
            self.is_selected = true;
            return true;
        } else if self.is_selected {
            match keyboard_press {
                '\n' => {
                    self.is_selected = false;
                }
                // Backspace
                '\u{0008}' => {
                    self.current_text.pop();
                }
                c => self.current_text.push(c),
            }

            return true;
        }

        return false;
    }

    fn rescale_after_split(&mut self, old_rect_data: RectData, new_rect_data: RectData) {
        self.abs_rect_data.top_left = self
            .abs_rect_data
            .top_left
            .move_to_new_rect(&old_rect_data, &new_rect_data);

        self.abs_rect_data = self
            .abs_rect_data
            .scale_dimensions(&old_rect_data, &new_rect_data);
    }

    fn rescale_after_move(&mut self, new_window_rect_data: RectData) {
        self.abs_rect_data = scale_rect_to_window(self.rel_rect_data, new_window_rect_data);
    }
}
