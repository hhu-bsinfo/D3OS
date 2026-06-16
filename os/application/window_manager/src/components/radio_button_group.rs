use alloc::{boxed::Box, rc::Rc};
use drawer::{rect_data::RectData, vertex::Vertex};

use crate::{
    components::container::{
        basic_container::BasicContainer,
        container_layout::{AlignmentMode, ContainerLayoutBuilder, FitMode, HorDirection},
        ContainerStylingBuilder,
    },
    signal::Stateful,
};

use super::{component::ComponentStyling, container::Container, radio_button::RadioButton};

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
        let mut button_container = BasicContainer::new(
            RectData {
                top_left: rel_center,
                width: 0,
                height: 0,
            },
            Some(
                ContainerLayoutBuilder::new()
                    .alignment(AlignmentMode::Horizontal(HorDirection::Left))
                    .fit(FitMode::GrowAndShrink)
                    .build(),
            ),
            Some(
                ContainerStylingBuilder::new()
                    .child_padding(spacing)
                    .show_border(false)
                    .build(),
            ),
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
