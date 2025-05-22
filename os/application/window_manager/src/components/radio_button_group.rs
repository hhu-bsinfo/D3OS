use alloc::{boxed::Box, rc::Rc, vec::Vec};
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::lfb::DEFAULT_CHAR_HEIGHT;
use spin::RwLock;
use terminal::DecodedKey;

use crate::{config::INTERACT_BUTTON, mouse_state::ButtonState, utils::scale_radius_to_rect, WindowManager};

use super::{component::{Casts, Component, ComponentStyling, Focusable, Interactable}, container::Container, radio_button::RadioButton};

pub struct RadioButtonGroup {
    id: usize,
    buttons: Vec<Rc<RwLock<RadioButton>>>,
    focused_button_index: usize,
    selected_button_index: Option<usize>,
    first_rel_center: Vertex,
    abs_radius: u32,
    rel_radius: u32,
    spacing: u32,
    on_change: Rc<Box<dyn Fn(usize) -> ()>>,
    styling: Option<ComponentStyling>,
}

impl RadioButtonGroup {
    pub fn new(
        num_buttons: usize,
        abs_center: Vertex,
        rel_center: Vertex,
        abs_radius: u32,
        rel_radius: u32,
        spacing: u32,
        selected_button_index: Option<usize>,
        on_change: Option<Box<dyn Fn(usize) -> ()>>,
        styling: Option<ComponentStyling>,
    ) -> Self {

        let buttons = (0..num_buttons)
            .map(|i| {
                Rc::new(RwLock::new(RadioButton::new(
                    abs_center.add(i as u32 * ((abs_radius * 2) + spacing), 0),
                    rel_center.add(i as u32 * ((rel_radius * 2) + spacing), 0),
                    abs_radius,
                    rel_radius,
                    selected_button_index == Some(i),
                    styling.clone(),
                )))
            })
            .collect();

        Self {
            id: WindowManager::generate_id(),
            buttons,
            selected_button_index,
            focused_button_index: 0,
            first_rel_center: rel_center,
            abs_radius,
            rel_radius,
            spacing,
            on_change: Rc::new(on_change.unwrap_or_else(|| Box::new(|_| {}))),
            styling,
        }
    }

    fn handle_click(&mut self) -> Option<Box<dyn FnOnce() -> ()>> {
        if let Some(selected_button_index) = self.selected_button_index {
            self.buttons.get(selected_button_index).unwrap().write().set_state(false);
        }

        self.selected_button_index = Some(self.focused_button_index);
        
        let on_change: Rc<Box<dyn Fn(usize)>> = Rc::clone(&self.on_change);
        let value = self.selected_button_index.unwrap();

        self.buttons.get(self.focused_button_index).unwrap().write().set_state(true);

        return Some(
            Box::new(move || {
                (on_change)(value);
            })
        );
    }
}

impl Component for RadioButtonGroup {
    fn draw(&mut self, focus_id: Option<usize>) {
        let is_focused = focus_id == Some(self.id);

        for (i, button) in self.buttons.iter().enumerate() {
            let is_button_focused = is_focused && i == self.focused_button_index;
            let child_id = button.read().get_id();
            
            button.write().draw(is_button_focused.then_some(child_id));
        }
    }

    fn is_dirty(&self) -> bool {
        self.buttons.iter().any(|button| button.read().is_dirty())
    }

    fn mark_dirty(&mut self) {
        self.buttons.iter().for_each(|button| button.write().mark_dirty());
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn get_abs_rect_data(&self) -> RectData {
        let first_rect = self.buttons.first().unwrap().read().get_abs_rect_data();

        let mut top_left = first_rect.top_left.clone();
        let mut bottom_right = first_rect.top_left.add(first_rect.width, first_rect.height);

        for button in &self.buttons {
            let rect_data = button.read().get_abs_rect_data();
            if rect_data.top_left.x < top_left.x {
                top_left.x = rect_data.top_left.x;
            }
            if rect_data.top_left.y < top_left.y {
                top_left.y = rect_data.top_left.y;
            }

            let curr_bottom_right: Vertex = Vertex {
                x: rect_data.top_left.x + rect_data.width,
                y: rect_data.top_left.y + rect_data.height,
            };

            if curr_bottom_right.x > bottom_right.x {
                bottom_right.x = curr_bottom_right.x;
            }
            if curr_bottom_right.y > bottom_right.y {
                bottom_right.y = curr_bottom_right.y;
            }
        }

        RectData {
            top_left,
            width: bottom_right.x - top_left.x,
            height: bottom_right.y - top_left.y,
        }
    }

    fn get_drawn_rect_data(&self) -> RectData {
        let first_rect = self.buttons.first().unwrap().read().get_drawn_rect_data();

        let mut top_left = first_rect.top_left.clone();
        let mut bottom_right = first_rect.top_left.add(first_rect.width, first_rect.height);

        for button in &self.buttons {
            let rect_data = button.read().get_drawn_rect_data();
            if rect_data.top_left.x < top_left.x {
                top_left.x = rect_data.top_left.x;
            }
            if rect_data.top_left.y < top_left.y {
                top_left.y = rect_data.top_left.y;
            }

            let curr_bottom_right: Vertex = Vertex {
                x: rect_data.top_left.x + rect_data.width,
                y: rect_data.top_left.y + rect_data.height,
            };

            if curr_bottom_right.x > bottom_right.x {
                bottom_right.x = curr_bottom_right.x;
            }
            if curr_bottom_right.y > bottom_right.y {
                bottom_right.y = curr_bottom_right.y;
            }
        }

        RectData {
            top_left,
            width: bottom_right.x - top_left.x,
            height: bottom_right.y - top_left.y,
        }
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        let abs_center = parent.scale_vertex_to_container(self.first_rel_center);

        self.abs_radius = scale_radius_to_rect(self.rel_radius, 7, parent.get_abs_rect_data());

        for (i, button) in self.buttons.iter_mut().enumerate() {
            button.write().set_center(abs_center.add(i as u32 * ((self.abs_radius * 2) + self.spacing), 0));
            button.write().set_radius(self.abs_radius);
        }
    }
}

impl Casts for RadioButtonGroup {
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

impl Focusable for RadioButtonGroup {
    fn focus(&mut self) {
        self.mark_dirty();
    }

    fn unfocus(&mut self) {
        self.mark_dirty();
    }
}

impl Interactable for RadioButtonGroup {
    fn consume_keyboard_press(&mut self, keyboard_press: DecodedKey) -> Option<Box<dyn FnOnce() -> ()>> {
        if keyboard_press == DecodedKey::Unicode('w') {
            if self.focused_button_index == self.buttons.len() - 1 {
                // kein Callback damit Window Manager das Event nicht registriert
                return None;
            } else {
                self.focused_button_index += 1;
                self.mark_dirty();
                
                // leeres Callback damit Window Manager andere Events blockiert
                return Some(Box::new(|| {
                }));
            }
        } else if keyboard_press == DecodedKey::Unicode('s') {
            if self.focused_button_index == 0 {
                // kein Callback damit Window Manager das Event nicht registriert
                return None;
            } else {
                self.focused_button_index -= 1;
                self.mark_dirty();

                // leeres Callback damit Window Manager andere Events blockiert
                return Some(Box::new(|| {
                }));
            }
        } else if keyboard_press == INTERACT_BUTTON {
            return self.handle_click();
        }

        return None;
    }

    fn consume_mouse_event(&mut self, mouse_event: &crate::mouse_state::MouseEvent) -> Option<Box<dyn FnOnce() -> ()>> {
        // Find the hovered radio button
        let hovered_button_index = self.buttons.iter().enumerate()
            .find_map(|(i, button)| {
            let rect_data = button.read().get_abs_rect_data();
            rect_data.contains_vertex(&Vertex::new(mouse_event.position.x, mouse_event.position.y))
                .then_some(i)
            });

        // Redraw radio group if neccessary
        if let Some(new_index) = hovered_button_index {
            if new_index != self.focused_button_index {
                self.focused_button_index = new_index;
                self.mark_dirty();
            }
        }

        // Check for mouse click
        if mouse_event.buttons.left.is_pressed() {
            return self.handle_click();
        }

        None
    }
}