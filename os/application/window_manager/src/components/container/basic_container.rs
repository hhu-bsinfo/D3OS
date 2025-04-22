use alloc::{format, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::color::Color;

use crate::{
    components::component::{Casts, Component},
    signal::ComponentRef,
    utils::scale_rect_to_window,
};

use super::Container;

pub struct BasicContainer {
    id: Option<usize>,
    childs: Vec<ComponentRef>,

    rel_rect_data: RectData,
    abs_rect_data: RectData,
    drawn_rect_data: RectData,

    is_dirty: bool,
}

impl BasicContainer {
    pub fn new(rel_rect_data: RectData, abs_rect_data: RectData) -> Self {
        Self {
            id: None,
            childs: Vec::new(),

            rel_rect_data,
            abs_rect_data,
            drawn_rect_data: abs_rect_data.clone(),

            is_dirty: true,
        }
    }
}

impl Container for BasicContainer {
    fn add_child(&mut self, child: ComponentRef) {
        self.childs.push(child);
    }
}

impl Component for BasicContainer {
    fn draw(&mut self, focus_id: Option<usize>) {
        let dirty_components = self
            .childs
            .iter()
            .filter(|child| child.read().is_dirty())
            .collect::<Vec<_>>();

        // Draw the border (DEBUG)
        if self.is_dirty {
            Drawer::draw_rectangle(
                self.abs_rect_data,
                Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                    alpha: 100,
                },
            );
    
            self.drawn_rect_data = self.abs_rect_data.clone();
        }

        if dirty_components.is_empty() && !self.is_dirty {
            return;
        }

        // Clear the area of dirty components (not needed, if the whole container is dirty)
        if !self.is_dirty {
            for child in &dirty_components {
                let rect_data = child.read().get_drawn_rect_data();
                Drawer::partial_clear_screen(rect_data);
            }
        }

        // Draw dirty child components
        for child in dirty_components {
            child.write().draw(focus_id);
        }

        self.is_dirty = false;
    }

    fn rescale_after_split(&mut self, old_window_rect: RectData, new_window_rect: RectData) {
        // TODO: This should project new_window_rect to abs_rect_data???
        let old_abs_rect_data = self.abs_rect_data;

        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window_rect,
            (10, 10),
            (1000, 1000),
            false,
            1.0,
        );

        terminal::write::log_debug(&format!(
            "Rescale after split ({}): abs_rect = {:?} -> {:?}, new_window_rect = {:?}",
            self.id.unwrap_or(0),
            old_abs_rect_data,
            self.abs_rect_data,
            new_window_rect
        ));

        //self.mark_dirty();
    }

    fn rescale_after_move(&mut self, new_window_rect: RectData) {
        // TODO: This should project new_window_rect to abs_rect_data???
        let old_abs_rect_data = self.abs_rect_data;

        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window_rect,
            (10, 10),
            (1000, 1000),
            false,
            1.0,
        );

        terminal::write::log_debug(&format!(
            "Rescale after move ({}): abs_rect = {:?} -> {:?}, new_window_rect = {:?}",
            self.id.unwrap_or(0),
            old_abs_rect_data,
            self.abs_rect_data,
            new_window_rect
        ));

        // Rescale all child components
        for child in &self.childs {
            child.write().rescale_after_move(new_window_rect);
        }

        //self.mark_dirty();
    }

    fn get_abs_rect_data(&self) -> RectData {
        todo!()
    }

    fn get_drawn_rect_data(&self) -> RectData {
        todo!()
    }

    /// Returns whether any child is dirty
    fn is_dirty(&self) -> bool {
        self.is_dirty || self.childs.iter().any(|child| child.read().is_dirty())
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
    }

    fn get_id(&self) -> Option<usize> {
        self.id
    }

    /// Marks all child components as dirty
    fn mark_dirty(&mut self) {
        self.childs
            .iter()
            .for_each(|child| child.write().mark_dirty());
        
        self.is_dirty = true;
    }
}

impl Casts for BasicContainer {}
