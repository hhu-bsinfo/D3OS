use alloc::{boxed::Box, rc::Rc, string::String};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{color::{Color, WHITE, YELLOW}, lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH}};
use spin::{rwlock::RwLock};

use crate::{signal::{ComponentRef, Signal, Stateful}, WindowManager, SCREEN};

use super::{component::{Casts, Component, ComponentStyling, Hideable}, container::Container};

pub const TEXT_COLOR: Color = WHITE;
pub const TEXT_COLOR_FOCUSED: Color = YELLOW;

pub struct Label {
    pub id: usize,
    pub is_dirty: bool,
    pub abs_pos: Vertex,
    pub rel_pos: Vertex,
    rel_font_size: usize,
    pub text: Stateful<String>,
    pub font_scale: (u32, u32),
    is_hidden: bool,
    drawn_rect_data: RectData,
    styling: ComponentStyling,
}

impl Label {
    pub fn new(
        rel_pos: Vertex,
        rel_font_size: usize,
        text: Stateful<String>,
        styling: Option<ComponentStyling>
    ) -> ComponentRef {
        let signal = text.clone();

        let label = Box::new(
            Self {
                id: WindowManager::generate_id(),
                is_dirty: true,
                abs_pos: Vertex::zero(),
                rel_pos,
                rel_font_size,
                text,
                font_scale: (1, 1),
                is_hidden: false,
                drawn_rect_data: RectData::zero(),
                styling: styling.unwrap_or_default(),
            }
        );

        let component: Rc<RwLock<Box<dyn Component>>> = Rc::new(RwLock::new(label));

        signal.register_component(Rc::clone(&component));

        component
    }
}

impl Component for Label {
    fn draw(&mut self, _focus_id: Option<usize>) {
        if !self.is_dirty {
            return;
        }

        if self.is_hidden {
            self.is_dirty = false;
            return;
        }

        let styling = &self.styling;
        let text_color = styling.text_color;

        let text = self.text.get();
        Drawer::draw_string(
            text,
            self.abs_pos,
            text_color,
            None,
            self.font_scale,
        );

        self.drawn_rect_data = self.get_abs_rect_data();

        self.is_dirty = false;
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        self.abs_pos = parent.scale_vertex_to_container(self.rel_pos);
        self.font_scale = parent.scale_font_to_container(self.rel_font_size);

        self.mark_dirty();
    }

    fn get_abs_rect_data(&self) -> RectData {
        RectData {
            top_left: self.abs_pos,
            width: self.text.get().len() as u32 * DEFAULT_CHAR_WIDTH * self.font_scale.0,
            height: DEFAULT_CHAR_HEIGHT * self.font_scale.1,
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
        self.drawn_rect_data
    }
}

impl Casts for Label {
    fn as_hideable(&self) -> Option<&dyn Hideable> {
        Some(self)
    }

    fn as_hideable_mut(&mut self) -> Option<&mut dyn Hideable> {
        Some(self)
    }
}

impl Hideable for Label {
    fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    fn hide(&mut self) {
        self.is_hidden = true;
        self.mark_dirty();
    }

    fn show(&mut self) {
        self.is_hidden = false;
        self.mark_dirty();
    }
}
