use graphic::bitmap::ScalingMode;
// Julius Drodofsky
use terminal::DecodedKey;

use super::component::{Casts, Component, ComponentStyling, Interactable};
use crate::components::component::*;
use crate::components::container::Container;
use crate::WindowManager;
use alloc::boxed::Box;
use alloc::rc::Rc;
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{bitmap::Bitmap, color::Color};
use spin::rwlock::RwLock;

pub struct Canvas {
    pub id: usize,
    is_dirty: bool,
    is_selected: bool,
    is_disabled: bool,
    abs_rect_data: RectData,
    rel_rect_data: RectData,
    orig_rect_data: RectData,
    drawn_rect_data: RectData,
    styling: ComponentStyling,
    buffer: Rc<RwLock<Bitmap>>,
    scaling_mode: ScalingMode,
    scale_factor: f64,
    // function to get user input
    input: Rc<Box<dyn Fn(DecodedKey) -> ()>>,
}

impl Canvas {
    pub fn new(
        styling: Option<ComponentStyling>,
        orig_rect_data: RectData,
        rel_rect_data: RectData,
        buffer: Rc<RwLock<Bitmap>>,
        scaling_mode: ScalingMode,
        input: Option<Box<dyn Fn(DecodedKey) -> ()>>,
    ) -> Self {
        Self {
            id: WindowManager::generate_id(),
            is_dirty: true,
            is_selected: false,
            is_disabled: false,
            drawn_rect_data: RectData::zero(),
            abs_rect_data: RectData::zero(),
            rel_rect_data,
            orig_rect_data,
            styling: styling.unwrap_or_default(),
            buffer: buffer,
            scaling_mode,
            scale_factor: 1.0,
            input: Rc::new(input.unwrap_or_else(|| Box::new(|_| {}))),
        }
    }
}

impl Component for Canvas {
    fn draw(&mut self, focus_id: Option<usize>) {
        if !self.is_dirty {
            return;
        }
        let is_focused = focus_id == Some(self.id);
        let styling = &self.styling;

        let border_color = if self.is_selected {
            styling.selected_border_color
        } else if is_focused {
            styling.focused_border_color
        } else {
            styling.border_color
        };
        // only small corner to save render time
        let border_data = RectData {
            top_left: Vertex {
                x: self.abs_rect_data.top_left.x - 2,
                y: self.abs_rect_data.top_left.y - 2,
            },
            width: 20,
            height: 20,
        };
        Drawer::draw_filled_rectangle(
            border_data,
            Color {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 0,
            },
            Some(border_color),
        );
        Drawer::draw_bitmap(self.abs_rect_data.top_left, &self.buffer.read());
        self.drawn_rect_data = self.abs_rect_data;
        self.is_dirty = false;
    }
    fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn get_abs_rect_data(&self) -> RectData {
        self.abs_rect_data
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        let styling: &ComponentStyling = &self.styling;
        let min_dim = (12, 12);
        let max_dim = (
            self.orig_rect_data.width * self.scale_factor as u32,
            self.orig_rect_data.height * self.scale_factor as u32,
        );

        self.abs_rect_data = parent.scale_to_container(
            self.rel_rect_data,
            min_dim,
            max_dim,
            styling.maintain_aspect_ratio,
        );

        {
            self.buffer
                .write()
                .scale_in_place(self.scaling_mode, self.abs_rect_data.width, self.abs_rect_data.height);
        }
        self.mark_dirty();
    }
}

impl Focusable for Canvas {
    fn can_unfocus(&self) -> bool {
        !self.is_selected
    }

    fn focus(&mut self) {
        self.is_selected = true;
        self.mark_dirty();
    }

    fn unfocus(&mut self) {
        self.is_selected = false;
        self.mark_dirty();
    }
}

impl Interactable for Canvas {
    fn consume_keyboard_press(
        &mut self,
        keyboard_press: DecodedKey,
    ) -> Option<Box<dyn FnOnce() -> ()>> {
        if self.is_disabled {
            return None;
        }
        self.mark_dirty();
        let input = Rc::clone(&self.input);
        return Some(Box::new(move || {
            (input)(keyboard_press);
        }));
    }

    fn consume_mouse_event(
        &mut self,
        mouse_event: &crate::mouse_state::MouseEvent,
    ) -> Option<Box<dyn FnOnce() -> ()>> {
        if mouse_event.buttons.left.is_pressed() {
            self.is_selected = !self.is_selected;
            self.mark_dirty();
        }

        None
    }
}

impl Casts for Canvas {
    fn as_disableable(&self) -> Option<&dyn Disableable> {
        None
    }

    fn as_hideable(&self) -> Option<&dyn Hideable> {
        None
    }

    fn as_focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }

    fn as_focusable_mut(&mut self) -> Option<&mut dyn Focusable> {
        Some(self)
    }

    fn as_interactable(&self) -> Option<&dyn Interactable> {
        Some(self)
    }

    fn as_disableable_mut(&mut self) -> Option<&mut dyn Disableable> {
        None
    }

    fn as_hideable_mut(&mut self) -> Option<&mut dyn Hideable> {
        None
    }

    fn as_interactable_mut(&mut self) -> Option<&mut dyn Interactable> {
        Some(self)
    }

    fn as_clearable_mut(&mut self) -> Option<&mut dyn Clearable> {
        None
    }
}

impl Resizable for Canvas {
    fn rescale(&mut self, scale_factor: f64) {
        self.scale_factor *= scale_factor;

        self.abs_rect_data.width = (f64::from(self.abs_rect_data.width) * scale_factor) as u32;
        self.abs_rect_data.height = (f64::from(self.abs_rect_data.height) * scale_factor) as u32;

        self.rel_rect_data.width = (f64::from(self.rel_rect_data.width) * scale_factor) as u32;
        self.rel_rect_data.height = (f64::from(self.rel_rect_data.height) * scale_factor) as u32;
        {
            self.buffer
                .write()
                .scale_in_place(self.scaling_mode ,self.abs_rect_data.width, self.abs_rect_data.height);
        }
        self.mark_dirty();
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.scale_factor = 1.0;

        let scale_factor_x = width as f64 / self.abs_rect_data.width as f64;
        let scale_factor_y = height as f64 / self.abs_rect_data.height as f64;

        self.abs_rect_data.width = width;
        self.abs_rect_data.height = height;

        self.orig_rect_data.width = width;
        self.orig_rect_data.height = height;

        self.rel_rect_data.width = self.rel_rect_data.width * scale_factor_x as u32;
        self.rel_rect_data.height = self.rel_rect_data.height * scale_factor_y as u32;

        {
            self.buffer
                .write()
                .scale_in_place(self.scaling_mode, self.abs_rect_data.width, self.abs_rect_data.height);
        }
        self.mark_dirty();
    }
}

impl Disableable for Canvas {
    fn disable(&mut self) {
        self.is_disabled = true;
        self.mark_dirty();
    }

    fn enable(&mut self) {
        self.is_disabled = false;
        self.mark_dirty();
    }

    fn is_disabled(&self) -> bool {
        self.is_disabled
    }
}
