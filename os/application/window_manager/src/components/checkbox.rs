use alloc::{
    boxed::Box, rc::Rc
};
use drawer::{drawer::Drawer, rect_data::RectData};
use graphic::lfb::DEFAULT_CHAR_HEIGHT;

use crate::{
    config::INTERACT_BUTTON, mouse_state::ButtonState, utils::scale_rect_to_window
};

use super::{component::{Casts, Component, ComponentStyling, Disableable, Focusable, Hideable, Interactable}, container::Container};

pub struct Checkbox {
    pub id: Option<usize>,
    pub is_dirty: bool,
    abs_rect_data: RectData,
    rel_rect_data: RectData,
    orig_rect_data: RectData,
    drawn_rect_data: RectData,
    pub state: bool,
    on_change: Rc<Box<dyn Fn(bool) -> ()>>,
    is_disabled: bool,
    is_hidden: bool,
    styling: ComponentStyling,
}

impl Checkbox {
    pub fn new(
        abs_rect_data: RectData,
        rel_rect_data: RectData,
        orig_rect_data: RectData,
        state: bool,
        on_change: Option<Box<dyn Fn(bool) -> ()>>,
        styling: Option<ComponentStyling>,
    ) -> Self {
        Self {
            id: None,
            is_dirty: true,
            abs_rect_data,
            rel_rect_data,
            orig_rect_data,
            drawn_rect_data: abs_rect_data.clone(),
            state,
            on_change: Rc::new(on_change.unwrap_or_else(|| Box::new(|_| {}))),
            is_disabled: false,
            is_hidden: false,
            styling: styling.unwrap_or_default(),
        }
    }

    fn handle_click(&mut self) -> Option<Box<dyn FnOnce() -> ()>> {
        self.state = !self.state;

        let on_change = Rc::clone(&self.on_change);
        let state = self.state;
        self.mark_dirty();

        return Some(Box::new(move || {
            (on_change)(state);
        }));
    }
}

impl Component for Checkbox {
    fn draw(&mut self, focus_id: Option<usize>) {
        if !self.is_dirty {
            return;
        }
        
        if self.is_hidden {
            self.is_dirty = false;
            return;
        }

        let styling = &self.styling;
        let is_focused = focus_id == self.id;

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

        if self.state {
            let RectData { top_left, width, height } = self.abs_rect_data;
            let bottom_right = top_left.add(width, height);
            let top_right = top_left.add(width, 0);
            let bottom_left = top_left.add(0, height);

            Drawer::draw_line(top_left, bottom_right, border_color);
            Drawer::draw_line(top_right, bottom_left, border_color);
        }

        self.is_dirty = false;
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        let styling: &ComponentStyling = &self.styling;
        
        let min_dim = (DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_HEIGHT);
        let max_dim = (self.orig_rect_data.width, self.orig_rect_data.height);

        self.abs_rect_data = parent.scale_to_container(
            self.rel_rect_data,
            min_dim,
            max_dim,
            styling.maintain_aspect_ratio,
        );

        self.mark_dirty();
    }
    
    fn get_abs_rect_data(&self) -> RectData {
        self.abs_rect_data
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
    }

    fn get_id(&self) -> Option<usize> {
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

impl Casts for Checkbox {
    fn as_disableable(&self) -> Option<&dyn Disableable> {
        Some(self)
    }

    fn as_hideable(&self) -> Option<&dyn Hideable> {
        Some(self)
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
        Some(self)
    }

    fn as_hideable_mut(&mut self) -> Option<&mut dyn Hideable> {
        Some(self)
    }

    fn as_interactable_mut(&mut self) -> Option<&mut dyn Interactable> {
        Some(self)
    }
}

impl Disableable for Checkbox {
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

impl Focusable for Checkbox {
    fn focus(&mut self) {
        self.mark_dirty();
    }

    fn unfocus(&mut self) -> bool {
        self.mark_dirty();
        true
    }
}

impl Interactable for Checkbox {
    fn consume_keyboard_press(&mut self, keyboard_press: char) -> Option<Box<dyn FnOnce() -> ()>> {
        if keyboard_press == INTERACT_BUTTON && !self.is_disabled {
            self.handle_click()
        } else {
            None
        }
    }

    fn consume_mouse_event(&mut self, mouse_event: &crate::mouse_state::MouseEvent) -> Option<Box<dyn FnOnce() -> ()>> {
        if mouse_event.buttons.left.is_pressed() && !self.is_disabled {
            self.handle_click()
        } else {
            None
        }
    }
}

impl Hideable for Checkbox {
    fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    fn hide(&mut self) {
        self.is_hidden = true;
        self.disable();
        self.mark_dirty();
    }

    fn show(&mut self) {
        self.is_hidden = false;
        self.enable();
        self.mark_dirty();
    }
}