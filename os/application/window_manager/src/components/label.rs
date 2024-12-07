use alloc::{boxed::Box, rc::Rc, string::String};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{color::{Color, WHITE, YELLOW}, lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH}};
use spin::{rwlock::RwLock};

use crate::{signal::{ComponentRef, Signal, Stateful}, utils::{scale_font, scale_pos_to_window}, SCREEN};

use super::component::{Casts, Component, ComponentStyling, Hideable};

pub const TEXT_COLOR: Color = WHITE;
pub const TEXT_COLOR_FOCUSED: Color = YELLOW;

pub struct Label {
    pub id: Option<usize>,
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
        abs_pos: Vertex,
        rel_pos: Vertex,
        rel_font_size: usize,
        text: Stateful<String>,
        font_scale: (u32, u32),
        styling: Option<ComponentStyling>
    ) -> ComponentRef {
        let signal = text.clone();

        let drawn_rect_data = RectData {
            top_left: abs_pos,
            width: text.get().len() as u32 * DEFAULT_CHAR_WIDTH * font_scale.0,
            height: DEFAULT_CHAR_HEIGHT * font_scale.1,
        };

        let label = Box::new(
            Self {
                id: None,
                is_dirty: true,
                abs_pos,
                rel_pos,
                rel_font_size,
                text,
                font_scale,
                is_hidden: false,
                drawn_rect_data,
                styling: styling.unwrap_or_default(),
            }
        );

        let component: Rc<RwLock<Box<dyn Component>>> = Rc::new(RwLock::new(label));

        signal.register_component(Rc::clone(&component));

        component
    }
}

impl Component for Label {
    fn draw(&mut self, is_focused: bool) {
        if !self.is_dirty {
            return;
        }

        if self.is_hidden {
            return;
        }

        let styling = self.styling;

        let text_color = if is_focused {
            styling.focused_border_color
        } else {
            styling.text_color
        };

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

    fn rescale_after_split(&mut self, old_window: RectData, new_window: RectData) {
        self.abs_pos = self.abs_pos.move_to_new_rect(&old_window, &new_window);
        self.font_scale = scale_font(&self.font_scale, &old_window, &new_window);
        self.mark_dirty();
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
        self.mark_dirty();
    }

    fn get_abs_rect_data(&self) -> RectData {
        RectData {
            top_left: self.abs_pos,
            width: self.text.get().len() as u32 * DEFAULT_CHAR_WIDTH * self.font_scale.0,
            height: DEFAULT_CHAR_HEIGHT * self.font_scale.1,
        }
    }

    fn get_id(&self) -> Option<usize> {
        self.id
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
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
