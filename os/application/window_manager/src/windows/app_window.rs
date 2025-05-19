use alloc::{boxed::Box, collections::LinkedList, rc::Rc, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::color::Color;
use hashbrown::HashMap;
use spin::RwLock;

use crate::{
    components::{component::Component, container::{basic_container::{BasicContainer, LayoutMode, StretchMode}, Container, ContainerStylingBuilder}}, config::{DEFAULT_FG_COLOR, FOCUSED_BG_COLOR}, signal::ComponentRef, utils::get_element_cursor_from_orderer, Interaction, WindowManager, SCREEN
};

pub const FOCUSED_INDICATOR_COLOR: Color = FOCUSED_BG_COLOR;
pub const FOCUSED_INDICATOR_LENGTH: u32 = 24;

/// This is the window used in workspaces to contains components from different apps
pub struct AppWindow {
    pub id: usize,
    pub rect_data: RectData,
    /// Indicates whether redrawing of this window is required in next loop-iteration
    pub is_dirty: bool,

    root_container: ComponentRef,
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

        // Root container that will hold all components
        let mut root_container = Box::new(BasicContainer::new(
            screen_rect,
            LayoutMode::None,
            StretchMode::None,
            Some(ContainerStylingBuilder::new().show_border(false).build()),
        ));

        // Initial scaling to window bounds
        root_container.move_to(rect_data);

        let root_container: ComponentRef = Rc::new(RwLock::new(root_container));

        Self {
            id: WindowManager::generate_id(),
            is_dirty: true,
            root_container,
            rect_data,
            focused_component: None,
        }
    }

    pub fn root_container(&self) -> ComponentRef {
        self.root_container.clone()
    }

    pub fn mark_window_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn insert_component(&mut self, new_component: ComponentRef, parent: ComponentRef) {
        // Add the component to the parent container
        parent.write().as_container_mut().expect("parent must be a container").add_child(new_component.clone());
    }

    fn focus_component(&mut self, comp: Option<ComponentRef>) {
        let focused_id = self.focused_component.as_ref().and_then(|comp| Some(comp.read().get_id()));
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

        //self.focused_component_id = None;
        self.focused_component = None;

        // Focus the new component
        if let Some(component) = comp {
            if let Some(focusable) = component.write().as_focusable_mut() {
                focusable.focus();
                self.focused_component = Some(component.clone());
            }
        }
    }

    pub fn interact_with_focused_component(&mut self, interaction: Interaction) -> bool {
        if let Some(focused_component) = &self.focused_component {
            //let focused_component = self.components.get(focused_component_id).unwrap();

            // prüfe ob Komponente interagierbar ist und bekomme Callback
            let callback: Option<Box<dyn FnOnce()>> = if let Some(interactable) = focused_component.write().as_interactable_mut() {
                //interactable.consume_keyboard_press(keyboard_press)
                match interaction {
                    Interaction::Keyboard(keyboard_press) => interactable.consume_keyboard_press(keyboard_press),
                    Interaction::Mouse(mouse_event) => interactable.consume_mouse_event(&mouse_event),
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

    pub fn focus_next_component(&mut self) {
        if !self.can_unfocus() {
            return;
        }

        let next_component = self.root_container.write()
            .as_container_mut().unwrap()
            .focus_next_child();

        self.focus_component(next_component);
    }

    pub fn focus_prev_component(&mut self) {
        if !self.can_unfocus() {
            return;
        }

        let prev_component = self.root_container.write()
            .as_container_mut().unwrap()
            .focus_prev_child();

        self.focus_component(prev_component);
    }

    pub fn focus_component_at(&mut self, pos: Vertex) {
        if !self.can_unfocus() {
            return;
        }

        let new_component = self.root_container.write().as_container_mut().unwrap().focus_child_at(pos);
        self.focus_component(new_component);
    }

    /// Rescales the window and marks it as dirty.
    pub fn rescale_window_in_place(&mut self, new_abs_rect: RectData) {
        self.root_container.write().as_container_mut().unwrap().move_to(new_abs_rect);

        self.mark_window_dirty();
    }

    /// Rescales and moves the window and marks it as dirty.
    pub fn rescale_window_after_move(&mut self, new_abs_rect: RectData) {
        self.rect_data = new_abs_rect;
        self.root_container.write().as_container_mut().unwrap().move_to(new_abs_rect);

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

        // "dirty" Komponenten werden gesammelt
        /*let dirty_components: Vec<_> = self.components.iter().filter(|component_entry| {
            component_entry.1.read().is_dirty() || self.is_dirty
        }).map(|(_, value)| value).collect();


        // keine Änderungen in Komponenten oder Fenster
        if dirty_components.is_empty() && !self.is_dirty {
            return;
        }*/

        let is_focused = self.id == focused_window_id;

        if self.is_dirty {
            Drawer::partial_clear_screen(self.rect_data);

            if is_focused {
                Drawer::draw_rectangle(self.rect_data, FOCUSED_BG_COLOR);
            } else {
                Drawer::draw_rectangle(self.rect_data, DEFAULT_FG_COLOR);
            }

            self.root_container.write().mark_dirty();
        }

        // es muss nicht teil bereinigt werden, falls das Fenster dirty ist da dies durch Splitting der Fall sein kann und so  in anderen Fenstern entstehen könnten
        /*if !self.is_dirty {  
            // bereinige zuvor gezeichnete Bereiche, der neu zu zeichnenden Komponenten
            for dirty_component in &dirty_components {
                Drawer::partial_clear_screen(dirty_component.read().get_drawn_rect_data());
            }
        }*/

        // Zeichne die aktualisierten Komponenten
        /*for dirty_component in &dirty_components {
            // This will mark non-dirty components as dirty, when window is dirty
            if self.is_dirty {
                dirty_component.write().mark_dirty();
            }

            // prüfe ob die Komponente fokussiert ist
            let is_focused = if let Some(focused_component_id) = self.focused_component_id {
                focused_component_id == dirty_component.read().get_id().unwrap()
            } else {
                false
            };

            dirty_component.write().draw(is_focused);
        }*/

        let focused_id = self.focused_component.as_ref().and_then(|comp| Some(comp.read().get_id()));
        self.root_container.write().draw(focused_id);

        self.is_dirty = false;
    }

    fn draw_is_focused_indicator(&self) {
        let top_left = self.rect_data.top_left;
        let side_length = FOCUSED_INDICATOR_LENGTH;
        let vertices = [
            top_left.add(1, 1),
            top_left.add(side_length, 1),
            top_left.add(1, side_length),
        ];
        Drawer::draw_filled_triangle(vertices, FOCUSED_INDICATOR_COLOR);
    }
}
