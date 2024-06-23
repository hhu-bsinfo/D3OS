use alloc::string::String;
use drawer::{drawer::Drawer, rect_data::RectData};
use graphic::color::{Color, CYAN};

use crate::{
    configs::general::{DEFAULT_FONT_SCALE, INTERACT_BUTTON},
    utils::scale_rect_to_window,
};

use super::component::Component;

const COLOR_SELECTED_BORDER: Color = CYAN;

pub struct InputField {
    /**
    If we are selected, all keyboard input is redirected to us, unless
    command-line-window is opened
    */
    is_selected: bool,
    max_chars: usize,
    workspace_index: usize,
    abs_rect_data: RectData,
    rel_rect_data: RectData,
    current_text: String,
}

impl InputField {
    pub fn new(
        workspace_index: usize,
        abs_rect_data: RectData,
        rel_rect_data: RectData,
        max_chars: usize,
    ) -> Self {
        Self {
            is_selected: false,
            max_chars,
            workspace_index,
            abs_rect_data,
            rel_rect_data,
            current_text: String::with_capacity(16),
        }
    }
}

impl Component for InputField {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        let rect_color = if self.is_selected {
            COLOR_SELECTED_BORDER
        } else {
            fg_color
        };

        Drawer::draw_rectangle(self.abs_rect_data, rect_color);
        Drawer::draw_string(
            self.current_text.clone(),
            self.abs_rect_data.top_left.add(2, 0),
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
                c => {
                    if self.current_text.len() < self.max_chars {
                        self.current_text.push(c);
                    }
                }
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
