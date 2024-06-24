use alloc::{rc::Rc, string::String};
use drawer::{drawer::Drawer, rect_data::RectData};
use graphic::{
    color::{Color, CYAN, WHITE},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use spin::RwLock;

use crate::{
    config::{BACKSPACE_UNICODE, INTERACT_BUTTON},
    utils::{scale_font, scale_rect_to_window},
};

use super::component::Component;

const COLOR_SELECTED_BORDER: Color = CYAN;
const COLOR_TEXT: Color = WHITE;

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
    rel_font_size: usize,
    font_scale: (u32, u32),
    current_text: Rc<RwLock<String>>,
}

impl InputField {
    pub fn new(
        workspace_index: usize,
        abs_rect_data: RectData,
        rel_rect_data: RectData,
        rel_font_size: usize,
        font_scale: (u32, u32),
        max_chars: usize,
        text: Rc<RwLock<String>>,
    ) -> Self {
        Self {
            is_selected: false,
            max_chars,
            workspace_index,
            abs_rect_data,
            rel_rect_data,
            rel_font_size,
            font_scale,
            current_text: text,
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
            self.current_text.read().clone(),
            self.abs_rect_data.top_left.add(2, 0),
            COLOR_TEXT,
            bg_color,
            self.font_scale,
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
                BACKSPACE_UNICODE => {
                    self.current_text.write().pop();
                }
                c => {
                    let mut text_lock = self.current_text.write();
                    if text_lock.len() < self.max_chars {
                        text_lock.push(c);
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

        let min_dim = (
            self.current_text.read().len() as u32 * DEFAULT_CHAR_WIDTH * self.font_scale.0,
            DEFAULT_CHAR_HEIGHT * self.font_scale.1,
        );

        self.abs_rect_data =
            self.abs_rect_data
                .scale_dimensions(&old_rect_data, &new_rect_data, Some(min_dim));

        self.font_scale = scale_font(&self.font_scale, &old_rect_data, &new_rect_data);
    }

    fn rescale_after_move(&mut self, new_window_rect_data: RectData) {
        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window_rect_data,
            (
                self.max_chars as u32 * DEFAULT_CHAR_WIDTH * self.font_scale.0,
                DEFAULT_CHAR_HEIGHT * self.font_scale.1,
            ),
        );

        self.font_scale = scale_font(
            &(self.rel_font_size as u32, self.rel_font_size as u32),
            &self.rel_rect_data,
            &self.abs_rect_data,
        );
    }
}
