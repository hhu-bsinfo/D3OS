use alloc::vec::Vec;
use drawer::rect_data::RectData;

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

    is_dirty: bool,
}

impl BasicContainer {
    pub fn new(rel_rect_data: RectData) -> Self {
        Self {
            id: None,
            childs: Vec::new(),

            rel_rect_data,
            abs_rect_data: rel_rect_data,

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
    fn draw(&mut self, is_focused: bool) {
        self.is_dirty = false;
    }

    fn rescale_after_split(
        &mut self,
        old_rect_data: RectData,
        new_rect_data: RectData,
    ) {
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

    fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
    }

    fn get_id(&self) -> Option<usize> {
        self.id
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }
}

impl Casts for BasicContainer {}
