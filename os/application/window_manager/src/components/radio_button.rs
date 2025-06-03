use alloc::{boxed::Box, rc::Rc};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use terminal::DecodedKey;

use crate::{
    config::INTERACT_BUTTON,
    mouse_state::MouseEvent,
    signal::{ComponentRef, ComponentRefExt, Stateful},
    WindowManager,
};

use super::{
    component::{Casts, Component, ComponentStyling, Focusable, Interactable},
    container::Container,
};

pub struct RadioButton {
    pub id: usize,

    abs_center: Vertex,
    rel_center: Vertex,
    abs_radius: u32,
    rel_radius: u32,
    drawn_rect_data: RectData,

    button_index: usize,
    selected_button_index: Stateful<usize>,
    on_change: Option<Rc<Box<dyn Fn(usize) -> ()>>>,

    is_disabled: bool,
    is_hidden: bool,
    is_dirty: bool,

    styling: ComponentStyling,
}

impl RadioButton {
    pub fn new(
        abs_center: Vertex,
        rel_center: Vertex,
        abs_radius: u32,
        rel_radius: u32,
        button_index: usize,
        selected_button_index: Stateful<usize>,
        on_change: Option<Rc<Box<dyn Fn(usize) -> ()>>>,
        styling: Option<ComponentStyling>,
    ) -> ComponentRef {
        let drawn_rect_data = RectData {
            top_left: abs_center.sub(abs_radius, abs_radius),
            width: abs_radius * 2,
            height: abs_radius * 2,
        };

        let radio_button = Box::new(Self {
            id: WindowManager::generate_id(),
            abs_center,
            rel_center,
            abs_radius,
            rel_radius,
            drawn_rect_data,

            button_index,
            selected_button_index: selected_button_index.clone(),
            on_change,

            is_disabled: false,
            is_hidden: false,
            is_dirty: true,
            styling: styling.unwrap_or_default(),
        });

        // Register the component in the signal
        let component = ComponentRef::from_component(radio_button);
        selected_button_index.register_component(component.clone());

        component
    }

    fn handle_click(&mut self) -> Option<Box<dyn FnOnce() -> ()>> {
        let button_index = self.button_index;
        let selected_button_index = self.selected_button_index.clone();
        let on_change = self.on_change.clone();

        return Some(Box::new(move || {
            if let Some(f) = on_change {
                f(button_index);
            }

            selected_button_index.set(button_index);
        }));
    }
}

impl Component for RadioButton {
    fn draw(&mut self, focus_id: Option<usize>) {
        // if !self.is_dirty {
        //     return;
        // }

        if self.is_hidden {
            self.is_dirty = false;
            return;
        }

        let styling = &self.styling;
        let is_focused = focus_id == Some(self.id);

        let bg_color = if self.is_disabled {
            styling.disabled_background_color
        } else if is_focused {
            styling.focused_background_color
        } else {
            styling.background_color
        };

        let border_color = if self.is_disabled {
            styling.disabled_border_color
        } else if is_focused {
            styling.focused_border_color
        } else {
            styling.border_color
        };

        Drawer::draw_circle(self.abs_center, self.abs_radius, border_color);

        self.drawn_rect_data = self.get_abs_rect_data();

        // Is the button selected?
        if self.selected_button_index.get() == self.button_index {
            let inner_rad = (self.abs_radius as f32 * 0.5) as u32;
            Drawer::draw_filled_circle(self.abs_center, inner_rad, border_color, None);
        }

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
        RectData {
            top_left: self.abs_center.sub(self.abs_radius, self.abs_radius),
            width: self.abs_radius * 2,
            height: self.abs_radius * 2,
        }
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        self.abs_center = parent.scale_vertex_to_container(self.rel_center);
    }
}

impl Focusable for RadioButton {
    fn focus(&mut self) {
        self.mark_dirty();
    }

    fn unfocus(&mut self) {
        self.mark_dirty();
    }
}

impl Interactable for RadioButton {
    fn consume_keyboard_press(
        &mut self,
        keyboard_press: DecodedKey,
    ) -> Option<Box<dyn FnOnce() -> ()>> {
        if keyboard_press == INTERACT_BUTTON {
            return self.handle_click();
        }

        return None;
    }

    fn consume_mouse_event(&mut self, mouse_event: &MouseEvent) -> Option<Box<dyn FnOnce() -> ()>> {
        if mouse_event.buttons.left.is_pressed() {
            return self.handle_click();
        }

        return None;
    }
}

impl Casts for RadioButton {
    fn as_focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }

    fn as_focusable_mut(&mut self) -> Option<&mut dyn Focusable> {
        Some(self)
    }

    fn as_interactable(&self) -> Option<&dyn Interactable> {
        Some(self)
    }

    fn as_interactable_mut(&mut self) -> Option<&mut dyn Interactable> {
        Some(self)
    }
}
