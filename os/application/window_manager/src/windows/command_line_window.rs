use alloc::string::String;
use drawer::{drawer::Drawer, rect_data::RectData};
use graphic::color::{CYAN, WHITE};

use crate::{config::DEFAULT_FONT_SCALE, ScreenSplitType};

/**
This is the window used to contain the command-line, which in turn is used
to run apps by their application-name
*/
pub struct CommandLineWindow {
    pub is_dirty: bool,
    /// If true, all keyboard input is redirected to typing in the name of the app
    pub enter_app_mode: bool,
    pub command: String,
    pub split_type: ScreenSplitType,
    rect_data: RectData,
}

impl CommandLineWindow {
    pub fn new(rect_data: RectData) -> Self {
        Self {
            is_dirty: true,
            rect_data,
            enter_app_mode: false,
            command: String::with_capacity(16),
            split_type: ScreenSplitType::Horizontal,
        }
    }

    pub fn activate_enter_app_mode(&mut self, split_type: ScreenSplitType) {
        self.enter_app_mode = true;
        self.is_dirty = true;
        self.split_type = split_type;
        self.command.clear();
    }

    pub fn deactivate_enter_app_mode(&mut self) {
        self.enter_app_mode = false;
    }

    pub fn push_char(&mut self, new_char: char) {
        self.command.push(new_char);
    }

    pub fn pop_char(&mut self) {
        self.command.pop();
    }
    pub fn draw(&mut self) {
        if !self.enter_app_mode || !self.is_dirty {
            return;
        }

        Drawer::partial_clear_screen(self.rect_data.sub_border());
        Drawer::draw_rectangle(self.rect_data, CYAN);
        Drawer::draw_string(
            self.command.clone(),
            self.rect_data.top_left.add(2, 2),
            WHITE,
            None,
            DEFAULT_FONT_SCALE,
        );

        self.is_dirty = false;
    }
}
