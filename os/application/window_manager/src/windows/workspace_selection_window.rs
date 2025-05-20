use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use drawer::{rect_data::RectData, vertex::Vertex};
use hashbrown::HashMap;

use crate::{
    components::{
        button::Button,
        component::{Casts, Component},
        container::{
            basic_container::{AlignmentMode, BasicContainer, LayoutMode, StretchMode},
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
    workspace_buttons: HashMap<usize, ComponentRef>,
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
            LayoutMode::Horizontal(AlignmentMode::Left),
            StretchMode::Fill,
            None,
        ));

        // Initial scaling to the window bounds
        root_container.move_to(abs_rect);

        Self {
            abs_rect,
            root_container,
            workspace_buttons: HashMap::new(),
        }
    }

    pub fn draw(&mut self, active_workspace_id: Option<usize>) {
        let focus_id = active_workspace_id
            .and_then(|workspace_id| self.workspace_buttons.get(&workspace_id))
            .map(|button| button.read().get_id());

        self.root_container
            .as_container_mut()
            .unwrap()
            .draw(focus_id);
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

        self.workspace_buttons.insert(workspace_id, button.clone());
        self.root_container.add_child(button);
    }

    pub fn unregister_workspace(&mut self, workspace_id: usize) {
        // Remove the button from the list
        let button = self.workspace_buttons.remove(&workspace_id);

        if let Some(button) = button {
            self.root_container
                .as_container_mut()
                .unwrap()
                .remove_child(button.read().get_id());
        }
    }

    pub fn handle_mouse_event(&mut self, mouse_event: &MouseEvent) -> Option<usize> {
        for (workspace_id, button) in self.workspace_buttons.iter_mut() {
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
                    return Some(*workspace_id);
                }
            }
        }

        None
    }

    pub fn mark_dirty(&mut self) {
        self.root_container.mark_dirty();
    }
}
