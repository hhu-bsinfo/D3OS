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
    /*id: usize,

    button_container: BasicContainer,

    focused_button_index: usize,
    selected_button_index: Stateful<usize>,
    first_rel_center: Vertex,
    abs_radius: u32,
    rel_radius: u32,
    spacing: u32,
    on_change: Rc<Box<dyn Fn(usize) -> ()>>,
    styling: Option<ComponentStyling>,*/
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
        on_change: Option<Rc<Box<dyn Fn(usize) -> ()>>>,
        styling: Option<ComponentStyling>,
    ) -> BasicContainer {
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

        // Create and add radio buttons
        for i in 0..num_buttons {
            let radio_button = RadioButton::new(
                abs_center.add(i as u32 * ((abs_radius * 2) + spacing), 0),
                rel_center.add(i as u32 * ((rel_radius * 2) + spacing), 0),
                abs_radius,
                rel_radius,
                i,
                selected_button_index.clone(),
                on_change.clone(),
                styling.clone(),
            );

            button_container.add_child(radio_button);
        }

        button_container
    }
}
