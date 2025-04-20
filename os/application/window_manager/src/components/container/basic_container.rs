use alloc::vec::Vec;
use drawer::{drawer::Drawer, rect_data::RectData};

use crate::{
    components::component::{Casts, Component},
    signal::ComponentRef,
};

use super::Container;

pub struct BasicContainer {
    id: Option<usize>,
    childs: Vec<ComponentRef>,

    rel_rect_data: RectData,
    abs_rect_data: RectData,
    //is_dirty: bool,
}

impl BasicContainer {
    pub fn new(rel_rect_data: RectData) -> Self {
        Self {
            id: None,
            childs: Vec::new(),

            rel_rect_data,
            abs_rect_data: rel_rect_data,
            //is_dirty: true,
        }
    }
}

impl Container for BasicContainer {
    fn add_child(&mut self, child: ComponentRef) {
        self.childs.push(child);
    }
}

impl Component for BasicContainer {
    fn draw(&mut self, _: bool) {
        let dirty_components = self
            .childs
            .iter()
            .filter(|child| child.read().is_dirty())
            .collect::<Vec<_>>();

        if dirty_components.is_empty() {
            return;
        }

        // Clear the area of dirty components
        for child in &dirty_components {
            let rect_data = child.read().get_drawn_rect_data();
            Drawer::partial_clear_screen(rect_data);
        }

        // Draw dirty child components
        for child in dirty_components {
            child.write().draw(false);
        }
    }

    fn rescale_after_split(&mut self, old_rect_data: RectData, new_rect_data: RectData) {
        todo!()
    }

    fn rescale_after_move(&mut self, new_rect_data: RectData) {
        todo!()
    }

    fn get_abs_rect_data(&self) -> RectData {
        todo!()
    }

    fn get_drawn_rect_data(&self) -> RectData {
        todo!()
    }

    /// Returns whether any child is dirty
    fn is_dirty(&self) -> bool {
        self.childs.iter().any(|child| child.read().is_dirty())
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
    }
}

impl Casts for BasicContainer {}
