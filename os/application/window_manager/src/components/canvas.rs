// Julius Drodofsky

use alloc::string::ToString;
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{bitmap::Bitmap, color::Color};
use super::component::{Casts, Component, ComponentStyling, Interactable};
use alloc::rc::Rc;
use spin::rwlock::RwLock;
use crate::components::container::Container;

pub struct Canvas {
    pub id: Option<usize>,
    is_dirty: bool,
    abs_rect_data: RectData,
    drawn_rect_data: RectData,
    styling: ComponentStyling,
    buffer: Rc<RwLock<Bitmap>>,
} 

impl Canvas {
    pub fn new (
    styling: Option<ComponentStyling>,
    abs_rect_data: RectData,
    buffer:  Rc<RwLock<Bitmap>>, 
    ) -> Self{
    let drawn_rect_data = RectData::zero();
    Self {
        id: None,
        is_dirty: true,
        drawn_rect_data: RectData::zero(),
        abs_rect_data,
        styling: styling.unwrap_or_default(),
        buffer: buffer,
        }
    }
     
}

impl Component for Canvas {
    fn draw(&mut self, focus_id: Option<usize>) {
        if !self.is_dirty{
            return;
        }
        Drawer::draw_bitmap(self.abs_rect_data.top_left, &self.buffer.read());
        self.drawn_rect_data = self.abs_rect_data;
        self.is_dirty = false;
    }
    fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    fn get_id(&self) -> Option<usize> {
        self.id
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
    }
    fn get_abs_rect_data(&self) -> RectData {
       self.abs_rect_data 
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }


    fn rescale_to_container(&mut self, parent: &dyn Container) {}

}


impl Casts for Canvas {}