/*
    The workspace selection window is displayed above the windows
    and allows the user to switch workspaces or create/close them.
*/

use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::color::{BLUE, RED};
use hashbrown::HashMap;

use crate::{
    components::{
        button::Button,
        component::{Casts, Component, ComponentStylingBuilder},
        container::{
            basic_container::{AlignmentMode, BasicContainer, LayoutMode, StretchMode},
            Container, ContainerStylingBuilder,
        },
    },
    mouse_state::MouseEvent,
    signal::{ComponentRef, ComponentRefExt, Signal},
    SCREEN,
};

pub enum WorkspaceSelectionEvent {
    None,
    Switch(usize),
    New,
    Close,
}

pub struct WorkspaceSelectionWindow {
    abs_rect: RectData,
    root_container: Box<BasicContainer>,
    button_container: ComponentRef,
    action_container: ComponentRef,

    new_workspace_button: ComponentRef,
    close_workspace_button: ComponentRef,

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
            LayoutMode::None,
            StretchMode::Fill,
            None,
        ));

        // Initial scaling to the window bounds
        root_container.move_to(abs_rect);

        // Container for the workspace buttons
        let button_container = Box::new(BasicContainer::new(
            RectData {
                top_left: Vertex::zero(),
                width: 0,
                height: 0,
            },
            LayoutMode::Horizontal(AlignmentMode::Left),
            StretchMode::Fill,
            Some(ContainerStylingBuilder::new().show_border(false).build()),
        ));

        // Container for workspace actions (new, close)
        let mut action_container = Box::new(BasicContainer::new(
            RectData {
                top_left: Vertex::zero(),
                width: 0,
                height: 0,
            },
            LayoutMode::Horizontal(AlignmentMode::Right),
            StretchMode::Fill,
            Some(ContainerStylingBuilder::new().show_border(false).build()),
        ));

        // Action buttons
        let rel_rect = RectData {
            top_left: Vertex::zero(),
            width: 50,
            height: 0,
        };

        let new_workspace_button = Button::new(
            rel_rect,
            rel_rect,
            Some(Signal::new(String::from("+"))),
            1,
            Some(Box::new(move || {})),
            Some(
                ComponentStylingBuilder::new()
                    .border_color(BLUE)
                    .background_color(BLUE.dim())
                    .build(),
            ),
        );

        let close_workspace_button = Button::new(
            rel_rect,
            rel_rect,
            Some(Signal::new(String::from("X"))),
            1,
            Some(Box::new(move || {})),
            Some(
                ComponentStylingBuilder::new()
                    .border_color(RED)
                    .background_color(RED.dim())
                    .build(),
            ),
        );

        action_container.add_child(close_workspace_button.clone());
        action_container.add_child(new_workspace_button.clone());

        let button_container = ComponentRef::from_component(button_container);
        let action_container = ComponentRef::from_component(action_container);

        root_container.add_child(button_container.clone());
        root_container.add_child(action_container.clone());

        Self {
            abs_rect,
            root_container,
            button_container,
            action_container,

            new_workspace_button,
            close_workspace_button,

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

    /// Adds a new buttons for the given workspace id
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
            Some(Box::new(move || {})),
            None,
        );

        self.workspace_buttons.insert(workspace_id, button.clone());
        self.button_container
            .write()
            .as_container_mut()
            .unwrap()
            .add_child(button);
    }

    /// Removes the designated button the the given workspace id
    pub fn unregister_workspace(&mut self, workspace_id: usize) {
        // Remove the button from the list
        let button = self.workspace_buttons.remove(&workspace_id);

        if let Some(button) = button {
            self.button_container
                .write()
                .as_container_mut()
                .unwrap()
                .remove_child(button.read().get_id());
        }
    }

    pub fn handle_mouse_event(&mut self, mouse_event: &MouseEvent) -> WorkspaceSelectionEvent {
        // New workspace button
        if self
            .new_workspace_button
            .read()
            .get_abs_rect_data()
            .contains_vertex(&mouse_event.position)
        {
            if self
                .new_workspace_button
                .write()
                .as_interactable_mut()
                .unwrap()
                .consume_mouse_event(mouse_event)
                .is_some()
            {
                return WorkspaceSelectionEvent::New;
            }
        }

        // Close workspace button
        if self
            .close_workspace_button
            .read()
            .get_abs_rect_data()
            .contains_vertex(&mouse_event.position)
        {
            if self
                .close_workspace_button
                .write()
                .as_interactable_mut()
                .unwrap()
                .consume_mouse_event(mouse_event)
                .is_some()
            {
                return WorkspaceSelectionEvent::Close;
            }
        }

        // Switch workspace
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
                    return WorkspaceSelectionEvent::Switch(*workspace_id);
                }
            }
        }

        WorkspaceSelectionEvent::None
    }

    pub fn mark_dirty(&mut self) {
        self.root_container.mark_dirty();
    }
}
