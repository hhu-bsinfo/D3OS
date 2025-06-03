use alloc::{boxed::Box, rc::Rc};
use drawer::{rect_data::RectData, vertex::Vertex};

use crate::{
    components::container::ContainerStylingBuilder, signal::Stateful
};

use super::{
    component::ComponentStyling,
    container::{
        basic_container::{AlignmentMode, BasicContainer, LayoutMode, StretchMode},
        Container,
    },
    radio_button::RadioButton,
};

pub struct RadioButtonGroup;

impl RadioButtonGroup {
    pub fn new(
        num_buttons: usize,
        rel_center: Vertex,
        rel_radius: u32,
        spacing: u32,
        selected_button_index: Stateful<usize>,
        on_change: Option<Rc<Box<dyn Fn(usize) -> ()>>>,
        styling: Option<ComponentStyling>,
    ) -> BasicContainer {
        // TODO: Implement a special kind of container for this
        let mut button_container = BasicContainer::new(
            RectData {
                top_left: rel_center,
                width: 100,
                height: 50,
            },
            LayoutMode::Horizontal(AlignmentMode::Left),
            StretchMode::None,
            Some(ContainerStylingBuilder::new().child_padding(spacing).show_border(true).build()),
        );

        // Create and add radio buttons
        for i in 0..num_buttons {
            let radio_button = RadioButton::new(
                Vertex::zero(),
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
