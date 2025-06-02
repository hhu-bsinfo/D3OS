use alloc::{boxed::Box, rc::Rc, vec::Vec};
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::lfb::DEFAULT_CHAR_HEIGHT;
use spin::RwLock;
use terminal::DecodedKey;

use crate::{
    config::INTERACT_BUTTON,
    mouse_state::ButtonState,
    signal::{ComponentRef, ComponentRefExt, Stateful},
    utils::scale_radius_to_rect,
    WindowManager,
};

use super::{
    component::{Casts, Component, ComponentStyling, Focusable, Interactable},
    container::{
        basic_container::{AlignmentMode, BasicContainer, LayoutMode, StretchMode},
        Container,
    },
    radio_button::RadioButton,
};

pub struct RadioButtonGroup {
    id: usize,

    button_container: BasicContainer,

    focused_button_index: usize,
    selected_button_index: Stateful<usize>,
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
        selected_button_index: Stateful<usize>,
        on_change: Option<Box<dyn Fn(usize) -> ()>>,
        styling: Option<ComponentStyling>,
    ) -> Self {
        let mut button_container = BasicContainer::new(
            RectData {
                top_left: rel_center,
                width: 150,
                height: 50,
            },
            LayoutMode::Horizontal(AlignmentMode::Left),
            StretchMode::None,
            None,
        );

        for i in 0..num_buttons {
            let radio_button = ComponentRef::from_component(Box::new(RadioButton::new(
                abs_center.add(i as u32 * ((abs_radius * 2) + spacing), 0),
                rel_center.add(i as u32 * ((rel_radius * 2) + spacing), 0),
                abs_radius,
                rel_radius,
                i,
                selected_button_index.clone(),
                styling.clone(),
            )));

            button_container.add_child(radio_button);
        }

        Self {
            id: WindowManager::generate_id(),

            button_container,

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
}

impl Component for RadioButtonGroup {
    fn draw(&mut self, focus_id: Option<usize>) {
        self.button_container.draw(focus_id);
    }

    fn is_dirty(&self) -> bool {
        self.button_container.is_dirty()
    }

    fn mark_dirty(&mut self) {
        self.button_container.mark_dirty();
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn get_abs_rect_data(&self) -> RectData {
        self.button_container.get_abs_rect_data()
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.button_container.get_drawn_rect_data()
    }

    fn rescale_to_container(&mut self, parent: &dyn Container) {
        self.button_container.rescale_to_container(parent);
    }
}

impl Casts for RadioButtonGroup {
    fn as_container(&self) -> Option<&dyn Container> {
        Some(&self.button_container)
    }

    fn as_container_mut(&mut self) -> Option<&mut dyn Container> {
        Some(&mut self.button_container)
    }
}
