use alloc::{boxed::Box, rc::Rc, string::{String, ToString}, sync::Arc, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{
    color::{Color, BLUE, WHITE},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use spin::{Mutex, RwLock};

use crate::observer::{Observable, Observer};

use super::component::Component;

pub const FG_COLOR: Color = WHITE;
pub const UNFOCUSED_BG_COLOR: Color = BLUE;

pub const WORKSPACE_SELECTION_LABEL_FONT_SCALE: (u32, u32) = (2, 2);
pub const HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW: u32 =
    DEFAULT_CHAR_HEIGHT * WORKSPACE_SELECTION_LABEL_FONT_SCALE.1 + 2;

pub struct SelectedWorkspaceLabel {
    pub pos: Vertex,
    pub text: String,
    pub tied_workspace: usize,
    state_dependencies: Vec<Rc<RwLock<Box<dyn Component>>>>,
}

impl SelectedWorkspaceLabel {
    pub fn new(pos: Vertex, text: String, tied_workspace: usize, state_dependencies: Vec<Rc<RwLock<Box<dyn Component>>>>) -> Self {
        Self {
            pos,
            text,
            tied_workspace,
            state_dependencies,
        }
    }
}

impl<'a> Component for SelectedWorkspaceLabel {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        Drawer::draw_string(
            self.text.to_string(),
            self.pos,
            fg_color,
            bg_color,
            WORKSPACE_SELECTION_LABEL_FONT_SCALE,
        );
    }

    fn consume_keyboard_press(&mut self, _keyboard_press: char) -> bool {
        return false;
    }

    fn rescale_after_split(&mut self, _old_window: RectData, _new_window: RectData) {
        // Should never be rescaled
    }

    fn rescale_after_move(&mut self, _new_rect_data: RectData) {
        // Should never be moved
    }

    fn get_abs_rect_data(&self) -> RectData {
        RectData {
            top_left: self.pos,
            width: self.text.len() as u32 * DEFAULT_CHAR_WIDTH * WORKSPACE_SELECTION_LABEL_FONT_SCALE.0,
            height: HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW,
        }
    }

    fn get_state_dependencies(&self) -> Vec<Rc<RwLock<Box<dyn Component>>>> {
        self.state_dependencies.clone()
    }
}
