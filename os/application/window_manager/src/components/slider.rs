use alloc::{boxed::Box, rc::Rc};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::lfb::DEFAULT_CHAR_HEIGHT;
use libm::roundf;
use crate::{config::DEFAULT_FONT_SCALE, mouse_state::ButtonState, utils::scale_rect_to_window};

use super::component::{Casts, Component, ComponentStyling, Disableable, Hideable, Interactable};

const HANDLE_WIDTH: u32 = 10;

pub struct Slider {
    id: Option<usize>,
    value: i32,
    min: i32,
    max: i32,
    abs_rect_data: RectData,
    rel_rect_data: RectData,
    orig_rect_data: RectData,
    drawn_rect_data: RectData,
    on_change: Rc<Box<dyn Fn(i32) -> ()>>,
    steps: u32,
    is_dirty: bool,

    // hideable
    is_hidden: bool,
    // disableable
    is_disabled: bool,

    styling: ComponentStyling,
}

impl Slider {
    pub fn new(
        abs_rect_data: RectData,
        rel_rect_data: RectData,
        orig_rect_data: RectData,
        on_change: Option<Box<dyn Fn(i32) -> ()>>,
        value: i32,
        min: i32,
        max: i32,
        steps: u32,
        styling: Option<ComponentStyling>,
    ) -> Self {
        Self {
            id: None,
            abs_rect_data,
            rel_rect_data,
            orig_rect_data,
            drawn_rect_data: abs_rect_data.clone(),
            on_change: Rc::new(on_change.unwrap_or_else(|| Box::new(|_| {}))),
            steps,
            value,
            min,
            max,
            is_dirty: true,
            is_disabled: false,
            is_hidden: false,
            styling: styling.unwrap_or_default(),
        }
    }

    pub fn on_change(&self, value: i32) {
        (self.on_change)(value);
    }

    fn update_value(&mut self, new_value: i32) -> Option<Box<dyn FnOnce() -> ()>> {
        if new_value > self.max || new_value < self.min {
            return None;
        }
        
        self.value = new_value;

        self.mark_dirty();
        
        let on_change = Rc::clone(&self.on_change);
        let value = self.value;

        Some(Box::new(move || {
            (on_change)(value);
        }))
    }
}

impl Component for Slider {
    fn draw(&mut self, is_focused: bool) {
        if !self.is_dirty {
            return;
        }
        
        if self.is_hidden {
            self.is_dirty = false;
            return;
        }

        let styling = &self.styling;

        let bg_color = if self.is_disabled {
            styling.disabled_background_color
        } else if is_focused {
            styling.focused_background_color
        } else {
            styling.background_color
        };

        let border_color = if is_focused {
            styling.focused_border_color
        } else if self.is_disabled {
            styling.disabled_border_color
        } else {
            styling.border_color
        };

        Drawer::draw_filled_rectangle(self.abs_rect_data, bg_color, Some(border_color));

        self.drawn_rect_data = self.abs_rect_data;

        let normalized_value = (self.value as f32 - self.min as f32) / (self.max as f32 - self.min as f32);
        let slider_position = roundf((self.abs_rect_data.width as f32 - 10 as f32) * normalized_value) as u32;

        let handle_rect = RectData {
            top_left: Vertex {
                x: self.abs_rect_data.top_left.x + slider_position as u32,
                y: self.abs_rect_data.top_left.y,
            },
            width: HANDLE_WIDTH,
            height: self.abs_rect_data.height,
        };
        
        Drawer::draw_filled_rectangle(handle_rect, styling.background_color, Some(styling.border_color));

        self.is_dirty = false;
    }

    fn rescale_after_split(&mut self, old_window: RectData, new_window: RectData) {
        let styling: &ComponentStyling = &self.styling;

        self.abs_rect_data.top_left = self
            .abs_rect_data
            .top_left
            .move_to_new_rect(&old_window, &new_window);

        let min_dim = (
            HANDLE_WIDTH * self.steps,
            DEFAULT_CHAR_HEIGHT
        );

        let aspect_ratio = self.orig_rect_data.width as f64 / self.orig_rect_data.height as f64;

        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window,
            min_dim,
            (self.orig_rect_data.width, self.orig_rect_data.height),
            styling.maintain_aspect_ratio,
            aspect_ratio,
        );

        self.mark_dirty();
    }

    fn rescale_after_move(&mut self, new_rect_data: RectData) {
        let styling: &ComponentStyling = &self.styling;

        let aspect_ratio = self.orig_rect_data.width as f64 / self.orig_rect_data.height as f64;

        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_rect_data,
            (HANDLE_WIDTH * self.steps, DEFAULT_CHAR_HEIGHT),
            (self.orig_rect_data.width, self.orig_rect_data.height),
            styling.maintain_aspect_ratio,
            aspect_ratio,
        );

        self.mark_dirty();
    }

    fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
    }

    fn get_id(&self) -> Option<usize> {
        self.id
    }

    fn get_abs_rect_data(&self) -> RectData {
        self.abs_rect_data
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }
}

impl Casts for Slider {
    fn as_disableable(&self) -> Option<&dyn Disableable> {
        Some(self)
    }

    fn as_hideable(&self) -> Option<&dyn Hideable> {
        Some(self)
    }

    fn as_interactable(&self) -> Option<&dyn Interactable> {
        Some(self)
    }

    fn as_disableable_mut(&mut self) -> Option<&mut dyn Disableable> {
        Some(self)
    }

    fn as_hideable_mut(&mut self) -> Option<&mut dyn Hideable> {
        Some(self)
    }

    fn as_interactable_mut(&mut self) -> Option<&mut dyn Interactable> {
        Some(self)
    }
}

impl Disableable for Slider {
    fn is_disabled(&self) -> bool {
        self.is_disabled
    }

    fn disable(&mut self) {
        self.is_disabled = true;
    }

    fn enable(&mut self) {
        self.is_disabled = false;
    }
}

impl Hideable for Slider {
    fn hide(&mut self) {
        self.is_hidden = true;
    }

    fn show(&mut self) {
        self.is_hidden = false;
    }

    fn is_hidden(&self) -> bool {
        self.is_hidden
    }
}

impl Interactable for Slider {
    fn consume_keyboard_press(&mut self, keyboard_press: char) -> Option<Box<dyn FnOnce() -> ()>> {
        if self.is_disabled {
            return None;
        }

        match keyboard_press {
            '+' => {
                let new_value = self.value + self.steps as i32;
                self.update_value(new_value)
            }
            '-' => {
                let new_value: i32 = self.value - self.steps as i32;
                self.update_value(new_value)
            }
            _ => {
                None
            }
        }
    }

    fn consume_mouse_event(&mut self, mouse_event: &crate::mouse_state::MouseEvent) -> Option<Box<dyn FnOnce() -> ()>> {
        if self.is_disabled {
            return None;
        }

        if mouse_event.button_states.left == ButtonState::Pressed || mouse_event.scroll > 0 {
            let new_value: i32 = self.value - self.steps as i32;
            return self.update_value(new_value);
        }

        if mouse_event.button_states.right == ButtonState::Pressed || mouse_event.scroll < 0 {
            let new_value: i32 = self.value + self.steps as i32;
            return self.update_value(new_value);
        }

        None
    }
}