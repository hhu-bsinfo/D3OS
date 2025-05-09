use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use drawer::{rect_data::RectData, vertex::Vertex};

use crate::{
    components::{
        button::Button,
        component::{Casts, Component},
        container::{
            basic_container::{BasicContainer, LayoutMode, StretchMode},
            Container,
        },
    },
    mouse_state::MouseEvent,
    signal::{ComponentRef, Signal},
    SCREEN,
};

pub struct WorkspaceSelectionWindow {
    abs_rect: RectData,
    root_container: Box<BasicContainer>,
    buttons: Vec<ComponentRef>,
}

impl WorkspaceSelectionWindow {
    pub fn new(abs_rect: RectData) -> Self {
        let screen_size = SCREEN.get().unwrap();
        let screen_rect = RectData {
            top_left: Vertex::zero(),
            width: screen_size.0,
            height: screen_size.1,
        };

        // Root container that will hold all buttons
        let mut root_container = Box::new(BasicContainer::new(
            screen_rect,
            LayoutMode::Horizontal,
            StretchMode::Fill,
            None,
        ));

        // Initial scaling to the window bounds
        root_container.move_to(abs_rect);

        Self {
            abs_rect,
            root_container,
            buttons: Vec::new(),
        }
    }

    pub fn draw(&mut self, active_workspace: Option<usize>) {
        self.root_container
            .as_container_mut()
            .unwrap()
            .draw(active_workspace);
    }

    pub fn register_workspace(&mut self, workspace_id: usize) {
        let rel_rect = RectData {
            top_left: Vertex::zero(),
            width: 50,
            height: 0,
        };

        let button = Button::new(
            rel_rect,
            rel_rect,
            Some(Signal::new(workspace_id.to_string())),
            1,
            (1, 1),
            Some(Box::new(move || {
                terminal::write::log_debug("click!");
            })),
            None,
        );

        button.write().set_id(workspace_id);

        self.root_container.add_child(button.clone());
        self.buttons.push(button);
    }

    pub fn unregister_workspace(&mut self, workspace_id: usize) {
        self.root_container
            .as_container_mut()
            .unwrap()
            .remove_child(workspace_id);

        // Remove the button from the list
        if let Some(index) = self
            .buttons
            .iter()
            .position(|button| button.read().get_id() == Some(workspace_id))
        {
            self.buttons.remove(index);
        }

        // Update all button ids...
        // HACK: This is bad, but it is required due to the current workspace system...
        self.buttons
            .iter_mut()
            .enumerate()
            .for_each(|(idx, button)| {
                button.write().set_id(idx);
            });
    }

    pub fn handle_mouse_event(&mut self, mouse_event: &MouseEvent) -> Option<usize> {
        for button in self.buttons.iter_mut() {
            let mut button = button.write();

            if button
                .get_abs_rect_data()
                .contains_vertex(&mouse_event.position)
            {
                let result = button
                    .as_interactable_mut()
                    .unwrap()
                    .consume_mouse_event(mouse_event);

                if result.is_some() {
                    return button.get_id();
                }
            }
        }

        None
    }

    pub fn mark_dirty(&mut self) {
        self.root_container.mark_dirty();
    }
}
