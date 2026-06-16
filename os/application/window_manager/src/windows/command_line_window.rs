use alloc::string::String;
use drawer::{drawer::Drawer, rect_data::RectData};
use terminal::DecodedKey;

use crate::{
    api::WindowManagerMessage,
    components::component::ComponentStyling,
    config::{BACKSPACE_UNICODE, DEFAULT_FG_COLOR, DEFAULT_FONT_SCALE, ESCAPE_UNICODE},
    ScreenSplitType, WindowManager,
};

/**
This is the window used to contain the command-line, which in turn is used
to run apps by their application-name
*/
pub struct CommandLineWindow {
    is_dirty: bool,
    /// If true, all keyboard input is redirected to typing in the name of the app
    enter_app_mode: bool,
    command: String,
    split_type: ScreenSplitType,
    rect_data: RectData,
    styling: ComponentStyling,
}

impl CommandLineWindow {
    pub fn new(rect_data: RectData, styling: Option<ComponentStyling>) -> Self {
        Self {
            is_dirty: true,
            rect_data,
            enter_app_mode: false,
            command: String::with_capacity(16),
            split_type: ScreenSplitType::Horizontal,
            styling: styling.unwrap_or_default(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.enter_app_mode
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

        let styling = &self.styling;

        let border_color = styling.selected_border_color;

        Drawer::partial_clear_screen(self.rect_data.sub_border());
        Drawer::draw_rectangle(self.rect_data, border_color);
        Drawer::draw_string(
            self.command.clone(),
            self.rect_data.top_left.add(2, 2),
            DEFAULT_FG_COLOR,
            None,
            DEFAULT_FONT_SCALE,
        );

        self.is_dirty = false;
    }

    /// Tries to process the given key and returns whether the command line wants
    /// to process further keys (`enter_app_mode`).
    pub fn process_keyboard_input(&mut self, keyboard_press: DecodedKey) -> bool {
        match keyboard_press {
            DecodedKey::Unicode('\n') => {
                WindowManager::get_api().send_message(WindowManagerMessage::LaunchApp(
                    self.command.clone(),
                    self.split_type,
                ));

                self.deactivate_enter_app_mode();
            }

            BACKSPACE_UNICODE => {
                self.is_dirty = true;
                self.pop_char();
            }

            ESCAPE_UNICODE => {
                self.deactivate_enter_app_mode();
            }

            DecodedKey::Unicode(c) => {
                self.is_dirty = true;
                self.push_char(c);
            }

            _ => (),
        }

        return self.enter_app_mode;
    }
}
