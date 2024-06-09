use alloc::string::String;
use concurrent::process::exit;
use drawer::drawer::{Drawer, RectData};
use graphic::color::{CYAN, WHITE};

use crate::SplitType;

/**
This is the window used to contain the command-line, which in turn is used
to run apps by their application-name
*/
pub struct CommandLineWindow {
    /// If true, all keyboard input is redirected to typing in the name of the app
    pub enter_app_mode: bool,
    pub command: String,
    pub is_visible: bool,
    pub split_type: SplitType,
    rect_data: RectData,
}

impl CommandLineWindow {
    pub fn new(rect_data: RectData) -> Self {
        Self {
            rect_data,
            enter_app_mode: false,
            command: String::with_capacity(16),
            is_visible: false,
            split_type: SplitType::Horizontal,
        }
    }

    pub fn activate_enter_app_mode(&mut self, split_type: SplitType) {
        self.enter_app_mode = true;
        self.split_type = split_type;
        self.is_visible = true;
        self.command.clear();
    }

    pub fn deactivate_enter_app_mode(&mut self) {
        self.enter_app_mode = false;
        self.is_visible = false;
    }

    pub fn push_char(&mut self, new_char: char) {
        self.command.push(new_char);
    }

    pub fn pop_char(&mut self) {
        self.command.pop();
    }

    pub fn draw(&mut self) {
        if !self.is_visible {
            return;
        }

        let RectData {
            top_left,
            width,
            height,
        } = self.rect_data;

        Drawer::draw_rectangle(top_left, top_left.add(width, height), CYAN);
        Drawer::draw_string(self.command.clone(), top_left.add(2, 2), WHITE);
    }
}
