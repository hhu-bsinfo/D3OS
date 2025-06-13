use alloc::{boxed::Box, string::String};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::color::{self, Color};

use crate::{
    api::WindowManagerMessage,
    components::{
        button::Button,
        component::ComponentStylingBuilder,
        container::{
            basic_container::{AlignmentMode, BasicContainer, LayoutMode, StretchMode},
            Container, ContainerStylingBuilder,
        },
    },
    config::{DEFAULT_FG_COLOR, FOCUSED_BG_COLOR},
    mouse_state::MouseEvent,
    signal::{ComponentRef, ComponentRefExt, Signal},
    Interaction, WindowManager, SCREEN,
};

pub const FOCUSED_INDICATOR_COLOR: Color = FOCUSED_BG_COLOR;
pub const FOCUSED_INDICATOR_LENGTH: u32 = 24;

/// This is the window used in workspaces to contains components from different apps
pub struct AppWindow {
    id: usize,
    pub rect_data: RectData,
    /// Indicates whether redrawing of this window is required in next loop-iteration
    is_dirty: bool,
    is_dragging: bool,

    root_container: ComponentRef,
    action_container: ComponentRef,
    component_container: ComponentRef,

    close_button: ComponentRef,
    next_button: ComponentRef,
    prev_button: ComponentRef,

    focused_component: Option<ComponentRef>,
}

impl AppWindow {
    pub fn new(rect_data: RectData) -> Self {
        let screen_size = SCREEN.get().unwrap();
        let screen_rect = RectData {
            top_left: Vertex::zero(),
            width: screen_size.0,
            height: screen_size.1,
        };

        // Root container for the window
        let mut root_container = Box::new(BasicContainer::new(
            screen_rect,
            LayoutMode::Vertical(AlignmentMode::Top),
            StretchMode::Fill,
            Some(
                ContainerStylingBuilder::new()
                    .show_border(false)
                    .child_padding(0)
                    .build(),
            ),
        ));

        // Action container for the window buttons
        let mut action_container = Box::new(BasicContainer::new(
            RectData {
                top_left: Vertex::zero(),
                width: 0,
                height: 40,
            },
            LayoutMode::Horizontal(AlignmentMode::Right),
            StretchMode::Fill,
            Some(ContainerStylingBuilder::new().show_border(true).show_background(true).build()),
        ));

        let button_rect = RectData {
            top_left: Vertex::zero(),
            width: 30,
            height: 0,
        };

        let close_button = Button::new(
            button_rect,
            button_rect,
            Some(Signal::new(String::from("X"))),
            1,
            Some(Box::new(move || {
                WindowManager::get_api().send_message(WindowManagerMessage::CloseCurrentWindow);
            })),
            Some(
                ComponentStylingBuilder::new()
                    .maintain_aspect_ratio(true)
                    .focused_border_color(color::RED)
                    .focused_background_color(color::RED.dim())
                    .build(),
            ),
        );

        let next_button = Button::new(
            button_rect,
            button_rect,
            Some(Signal::new(String::from(">"))),
            1,
            Some(Box::new(move || {
                WindowManager::get_api()
                    .send_message(WindowManagerMessage::MoveCurrentWindowForward);
            })),
            Some(
                ComponentStylingBuilder::new()
                    .maintain_aspect_ratio(true)
                    .build(),
            ),
        );

        let prev_button = Button::new(
            button_rect,
            button_rect,
            Some(Signal::new(String::from("<"))),
            1,
            Some(Box::new(move || {
                WindowManager::get_api()
                    .send_message(WindowManagerMessage::MoveCurrentWindowBackward);
            })),
            Some(
                ComponentStylingBuilder::new()
                    .maintain_aspect_ratio(true)
                    .build(),
            ),
        );

        action_container.add_child(close_button.clone());
        action_container.add_child(next_button.clone());
        action_container.add_child(prev_button.clone());

        let action_container = ComponentRef::from_component(action_container);

        // Component container that holds all API components
        let component_container = ComponentRef::from_component(Box::new(BasicContainer::new(
            RectData {
                top_left: Vertex::zero(),
                width: 0,
                height: 560,
            },
            LayoutMode::None,
            StretchMode::None,
            Some(ContainerStylingBuilder::new().show_border(false).build()),
        )));

        root_container.add_child(ComponentRef::clone(&action_container));
        root_container.add_child(ComponentRef::clone(&component_container));

        // Initial scaling to window bounds
        root_container.move_to(rect_data);
        let root_container = ComponentRef::from_component(root_container);

        Self {
            id: WindowManager::generate_id(),
            is_dirty: true,
            is_dragging: false,
            root_container,
            action_container,
            component_container,
            close_button,
            next_button,
            prev_button,
            rect_data,
            focused_component: None,
        }
    }

    pub fn get_id(&self) -> usize {
        self.id
    }

    pub fn root_container(&self) -> ComponentRef {
        //self.root_container.clone()
        self.component_container.clone()
    }

    pub fn mark_window_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn insert_component(&mut self, new_component: ComponentRef, parent: ComponentRef) {
        // Add the component to the parent container
        parent
            .write()
            .as_container_mut()
            .expect("parent must be a container")
            .add_child(new_component.clone());
    }

    fn focus_component(&mut self, comp: Option<ComponentRef>) {
        let focused_id = self
            .focused_component
            .as_ref()
            .and_then(|comp| Some(comp.read().get_id()));
        let new_id = comp.as_ref().and_then(|comp| Some(comp.read().get_id()));

        if focused_id == new_id {
            return;
        }

        // Unfocus old component
        if let Some(component) = &self.focused_component {
            if let Some(focusable) = component.write().as_focusable_mut() {
                // Does the component accept the unfocus?
                if !focusable.can_unfocus() {
                    return;
                }

                focusable.unfocus();
            }
        }

        self.focused_component = None;

        // Focus the new component
        if let Some(component) = comp {
            if let Some(focusable) = component.write().as_focusable_mut() {
                focusable.focus();
                self.focused_component = Some(component.clone());
            }
        }
    }

    /// Passes the interaction to the focused component and returns, whether the interaction has been handled.
    pub fn interact_with_focused_component(&mut self, interaction: Interaction) -> bool {
        if let Some(focused_component) = &self.focused_component {
            // prüfe ob Komponente interagierbar ist und bekomme Callback
            let callback: Option<Box<dyn FnOnce()>> =
                if let Some(interactable) = focused_component.write().as_interactable_mut() {
                    match interaction {
                        Interaction::Keyboard(keyboard_press) => {
                            interactable.consume_keyboard_press(keyboard_press)
                        }
                        Interaction::Mouse(mouse_event) => {
                            interactable.consume_mouse_event(&mouse_event)
                        }
                    }
                } else {
                    None
                };

            // führe Callback aus
            if let Some(callback) = callback {
                callback();
                return true;
            }
        }

        return false;
    }

    /// Returns whether the focused component wants to hold the focus.
    /// The window should not be unfocused as long as that's the case.
    pub fn can_unfocus(&self) -> bool {
        if let Some(focused) = &self.focused_component {
            if let Some(focusable) = focused.read().as_focusable() {
                return focusable.can_unfocus();
            }
        }

        return true;
    }

    /// Force unfocus the currently focused component
    pub fn unfocus(&mut self) {
        self.focused_component = None;

        if let Some(focused) = &self.focused_component {
            if let Some(focusable) = focused.write().as_focusable_mut() {
                return focusable.unfocus();
            }
        }
    }

    pub fn focus_next_component(&mut self) {
        if !self.can_unfocus() {
            return;
        }

        let next_component = self
            .root_container
            .write()
            .as_container_mut()
            .unwrap()
            .focus_next_child();

        self.focus_component(next_component);
    }

    pub fn focus_prev_component(&mut self) {
        if !self.can_unfocus() {
            return;
        }

        let prev_component = self
            .root_container
            .write()
            .as_container_mut()
            .unwrap()
            .focus_prev_child();

        self.focus_component(prev_component);
    }

    pub fn focus_component_at(&mut self, pos: Vertex) {
        if !self.can_unfocus() {
            return;
        }

        let new_component = self
            .root_container
            .write()
            .as_container_mut()
            .unwrap()
            .focus_child_at(pos);

        self.focus_component(new_component);
    }

    /// Rescales the window and marks it as dirty.
    pub fn rescale_window_in_place(&mut self, new_abs_rect: RectData) {
        self.root_container
            .write()
            .as_container_mut()
            .unwrap()
            .move_to(new_abs_rect);

        self.mark_window_dirty();
    }

    /// Rescales and moves the window and marks it as dirty.
    pub fn rescale_window_after_move(&mut self, new_abs_rect: RectData) {
        self.rect_data = new_abs_rect;
        self.root_container
            .write()
            .as_container_mut()
            .unwrap()
            .move_to(new_abs_rect);

        self.mark_window_dirty();
    }

    pub fn merge(&mut self, other_window: &mut AppWindow) {
        let _old_rect @ RectData {
            top_left: old_top_left,
            width: old_width,
            height: old_height,
        } = self.rect_data;
        let other_top_left = other_window.rect_data.top_left;
        let mut new_top_left = old_top_left;
        let mut new_width = old_width;
        let mut new_height = old_height;

        // We have a vertical splittype, the € mark the different top_left.x coords
        // €########€#########      ###################
        // #        |        #      #                 #
        // #        |        #      #                 #
        // #        |        # ===> #                 #
        // #        |        #      #                 #
        // #        |        #      #                 #
        // ###################      ###################
        if old_top_left.x != other_top_left.x {
            assert_eq!(old_top_left.y, other_top_left.y);
            new_top_left = Vertex::new(u32::min(old_top_left.x, other_top_left.x), old_top_left.y);
            new_width = old_width * 2;
        }
        // We have a horizontal splittype, the € mark the different top_left.y coords
        // €##################      ###################
        // #                 #      #                 #
        // #                 #      #                 #
        // €-----------------# ===> #                 #
        // #                 #      #                 #
        // #                 #      #                 #
        // ###################      ###################
        else if old_top_left.y != other_top_left.y {
            assert_eq!(old_top_left.x, other_top_left.x);
            new_top_left = Vertex::new(old_top_left.x, u32::min(old_top_left.y, other_top_left.y));
            new_height = old_height * 2;
        }

        self.rect_data = RectData {
            top_left: new_top_left,
            width: new_width,
            height: new_height,
        };

        self.rescale_window_in_place(self.rect_data);

        other_window.mark_window_dirty();
    }

    pub fn draw(&mut self, focused_window_id: usize, full: bool) {
        if full {
            self.is_dirty = true;
        }

        let is_focused = self.id == focused_window_id;

        // Clear the entire window if it is dirty
        if self.is_dirty {
            Drawer::partial_clear_screen(self.rect_data);
            self.root_container.write().mark_dirty();
        }

        // Draw components
        let focused_id = self
            .focused_component
            .as_ref()
            .and_then(|comp| Some(comp.read().get_id()));
        self.root_container.write().draw(focused_id);

        // Draw window border
        if self.is_dirty {
            if is_focused {
                Drawer::draw_rectangle(self.rect_data, FOCUSED_BG_COLOR);
            } else {
                Drawer::draw_rectangle(self.rect_data, DEFAULT_FG_COLOR);
            }
        }

        self.is_dirty = false;
    }
}
