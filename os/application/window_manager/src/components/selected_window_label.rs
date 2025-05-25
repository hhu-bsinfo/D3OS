use alloc::{boxed::Box, rc::Rc, string::{String, ToString}, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{
    color::{Color, BLUE, WHITE},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use spin::RwLock;

use crate::{config::FOCUSED_BG_COLOR, signal::ComponentRef, WindowManager};

use super::{component::{Casts, Component, ComponentStyling, Hideable}, container::Container};

pub const FG_COLOR: Color = WHITE;

pub const WORKSPACE_SELECTION_LABEL_FONT_SCALE: (u32, u32) = (2, 2);
pub const HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW: u32 =
    DEFAULT_CHAR_HEIGHT * WORKSPACE_SELECTION_LABEL_FONT_SCALE.1 + 2;

// wird nicht mehr genutzt
pub struct SelectedWorkspaceLabel {
    pub id: usize,
    pub is_dirty: bool,
    pub pos: Vertex,
    pub text: String,
    pub tied_workspace: usize,
    state_dependencies: Vec<ComponentRef>,
    is_hidden: bool,
    styling: ComponentStyling,
}

impl SelectedWorkspaceLabel {
    pub fn new(
        pos: Vertex,
        text: String,
        tied_workspace: usize,
        state_dependencies: Vec<ComponentRef>,
        styling: Option<ComponentStyling>,
    ) -> Self {
        Self {
            id: WindowManager::generate_id(),
            is_dirty: true,
            pos,
            text,
            tied_workspace,
            state_dependencies,
            is_hidden: false,
            styling: styling.unwrap_or_default(),
            
        }
    }
}

impl Component for SelectedWorkspaceLabel {
    fn draw(&mut self, focus_id: Option<usize>) {
        if !self.is_dirty {
            return;
        }
        
        if self.is_hidden {
            self.is_dirty = false;
            return;
        }

        let styling = &self.styling;
        let is_focused = focus_id == Some(self.id);

        let bg_color = if is_focused {
            styling.focused_background_color
        } else {
            styling.background_color
        };

        let text_color = if is_focused {
            styling.focused_text_color
        } else {
            styling.text_color
        };

        Drawer::draw_string(
            self.text.to_string(),
            self.pos,
            text_color,
            Some(bg_color),
            WORKSPACE_SELECTION_LABEL_FONT_SCALE,
        );

        self.is_dirty = false;
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        // Should never be called
    }

    fn get_abs_rect_data(&self) -> RectData {
        RectData {
            top_left: self.pos,
            width: self.text.len() as u32 * DEFAULT_CHAR_WIDTH * WORKSPACE_SELECTION_LABEL_FONT_SCALE.0,
            height: HEIGHT_WORKSPACE_SELECTION_LABEL_WINDOW,
        }
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.get_abs_rect_data()
    }
}

impl Casts for SelectedWorkspaceLabel {
    fn as_hideable(&self) -> Option<&dyn Hideable> {
        Some(self)
    }

    fn as_hideable_mut(&mut self) -> Option<&mut dyn Hideable> {
        Some(self)
    }
}

impl Hideable for SelectedWorkspaceLabel {
    fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    fn show(&mut self) {
        self.is_hidden = false;
        self.mark_dirty();
    }

    fn hide(&mut self) {
        self.is_hidden = true;
        self.mark_dirty();
    }
}
