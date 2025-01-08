use alloc::{boxed::Box, collections::LinkedList, rc::Rc, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::color::Color;
use hashbrown::HashMap;
use spin::RwLock;

use crate::{
    components::component::Component, config::{DEFAULT_FG_COLOR, FOCUSED_BG_COLOR}, utils::get_element_cursor_from_orderer, WindowManager
};

pub const FOCUSED_INDICATOR_COLOR: Color = FOCUSED_BG_COLOR;
pub const FOCUSED_INDICATOR_LENGTH: u32 = 24;

/// This is the window used in workspaces to contains components from different apps
pub struct AppWindow {
    pub id: usize,
    pub rect_data: RectData,
    /// Indicates whether redrawing of this window is required in next loop-iteration
    pub is_dirty: bool,
    components: HashMap<usize, Rc<RwLock<Box<dyn Component>>>>,
    /// focusable components are stored additionally in ordered fashion in here
    component_orderer: LinkedList<usize>,
    focused_component_id: Option<usize>,
}

impl AppWindow {
    pub fn new(id: usize, rect_data: RectData) -> Self {
        Self {
            id,
            is_dirty: true,
            components: HashMap::new(),
            component_orderer: LinkedList::new(),
            rect_data,
            focused_component_id: None,
        }
    }

    pub fn mark_component_dirty(&mut self, id: usize) {
        let component = self.components.get(&id).unwrap();
        component.write().mark_dirty();
    }

    pub fn mark_window_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn insert_component(&mut self, new_component: Rc<RwLock<Box<dyn Component>>>, is_focusable: bool) {
        let id = WindowManager::generate_id();
        
        if is_focusable {
            self.component_orderer.push_back(id);
            
            // Focus new (focusable) component, if it is the first one in the window
            if self.component_orderer.len() == 1 {
                self.focused_component_id = Some(id);
            }
        }
        
        new_component.write().set_id(id);
        self.components.insert(id, new_component);
    }

    pub fn interact_with_focused_component(&mut self, keyboard_press: char) -> bool {
        if let Some(focused_component_id) = &self.focused_component_id {
            let focused_component = self.components.get(focused_component_id).unwrap();

            // prüfe ob Komponente interagierbar ist und bekomme Callback
            let callback: Option<Box<dyn FnOnce()>> = if let Some(interactable) = focused_component.write().as_interactable_mut() {
                interactable.consume_keyboard_press(keyboard_press)
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

    pub fn focus_next_component(&mut self) {
        if let Some(focused_component_id) = self.focused_component_id {
            // Sicherheitszählung, um nicht endlos nach nicht versteckten Komponenten zu suchen 
            let mut iterations = 0;
            let total_components = self.component_orderer.len();
            
            let mut cursor =
                get_element_cursor_from_orderer(&mut self.component_orderer, focused_component_id)
                    .unwrap();
            

            loop {
                cursor.move_next();

                if cursor.current().is_none() {
                    cursor.move_next();
                }

                iterations += 1;

        
                if let Some(next_focused_el) = cursor.current() {
                    if let Some(component) = self.components.get(next_focused_el) {
                        if let Some(hideable) = component.read().as_hideable() {
                            // überspringe versteckte Komponenten
                            if hideable.is_hidden() {
                                continue;
                            }
                        }
                    }

                    self.focused_component_id = Some(next_focused_el.clone());
                    break;
                }

                // Alle Komponenten sind versteckt
                if iterations >= total_components {
                    self.focused_component_id = None; // Kein Fokus möglich
                    break;
                }
            }

            // markiere zuvor und neu fokusierte Komponente als dirty um Fokus-Indikator zu aktualisieren
            if let Some(next_focused_component_id) = self.focused_component_id {
                self.mark_component_dirty(focused_component_id);
                self.mark_component_dirty(next_focused_component_id);

            }
        }
    }

    pub fn focus_prev_component(&mut self) {
        if let Some(focused_component_id) = self.focused_component_id {
            // Sicherheitszählung, um nicht endlos nach nicht versteckten Komponenten zu suchen 
            let mut iterations = 0;
            let total_components = self.component_orderer.len();
            
            let mut cursor =
                get_element_cursor_from_orderer(&mut self.component_orderer, focused_component_id)
                    .unwrap();
            

            loop {
                cursor.move_prev();

                if cursor.current().is_none() {
                    cursor.move_prev();
                }

                iterations += 1;

        
                if let Some(prev_focused_el) = cursor.current() {
                    if let Some(component) = self.components.get(prev_focused_el) {
                        if let Some(hideable) = component.read().as_hideable() {
                            // überspringe versteckte Komponenten
                            if hideable.is_hidden() {
                                continue;
                            }
                        }
                    }

                    self.focused_component_id = Some(prev_focused_el.clone());
                    break;
                }

                // Alle Komponenten sind versteckt
                if iterations >= total_components {
                    self.focused_component_id = None; // Kein Fokus möglich
                    break;
                }
            }

            // markiere zuvor und neu fokusierte Komponente als dirty um Fokus-Indikator zu aktualisieren
            if let Some(next_focused_component_id) = self.focused_component_id {
                self.mark_component_dirty(focused_component_id);
                self.mark_component_dirty(next_focused_component_id);

            }
        }
    }

    pub fn rescale_window_in_place(&mut self, old_rect_data: RectData, new_rect_data: RectData) {
        let components = self.components.values();
        for component in components {
            component.write().mark_dirty();
            component.write().rescale_after_split(old_rect_data, new_rect_data);
        }

        self.mark_window_dirty();
    }

    pub fn rescale_window_after_move(&mut self, new_rect_data: RectData) {
        self.rect_data = new_rect_data;

        for component in self.components.values_mut() {
            component.write().rescale_after_move(new_rect_data);
        }
    }

    pub fn merge(&mut self, other_window: &mut AppWindow) {
        let old_rect @ RectData {
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

        self.rescale_window_in_place(old_rect, self.rect_data);
        self.mark_window_dirty();
        other_window.mark_window_dirty();
    }

    pub fn draw(&mut self, focused_window_id: usize, full: bool) {
        if full {
            self.is_dirty = true;
        }

        // "dirty" Komponenten werden gesammelt
        let dirty_components: Vec<_> = self.components.iter().filter(|component_entry| {
            component_entry.1.read().is_dirty() || self.is_dirty
        }).map(|(_, value)| value).collect();


        // keine Änderungen in Komponenten oder Fenster
        if dirty_components.is_empty() && !self.is_dirty {
            return;
        }

        let is_focused = self.id == focused_window_id;

        if self.is_dirty {
            Drawer::partial_clear_screen(self.rect_data);

            if is_focused {
                Drawer::draw_rectangle(self.rect_data, FOCUSED_BG_COLOR);
            } else {
                Drawer::draw_rectangle(self.rect_data, DEFAULT_FG_COLOR);
            }
        }

        // es muss nicht teil bereinigt werden, falls das Fenster dirty ist da dies durch Splitting der Fall sein kann und so  in anderen Fenstern entstehen könnten
        if !self.is_dirty {  
            // bereinige zuvor gezeichnete Bereiche, der neu zu zeichnenden Komponenten
            for dirty_component in &dirty_components {
                Drawer::partial_clear_screen(dirty_component.read().get_drawn_rect_data());
            }
        }

        // Zeichne die aktualisierten Komponenten
        for dirty_component in &dirty_components {
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
        }

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
