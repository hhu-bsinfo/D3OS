// Julius Drodofsky

use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use super::component::{Casts, Component, ComponentStyling, Interactable};
use alloc::vec::Vec;
use alloc::rc::Rc;
use spin::rwlock::RwLock;
use crate::components::container::Container;

pub struct Canvas {
    pub id: Option<usize>,
    is_dirty: bool,
    abs_pos: Vertex,
    rel_pos: Vertex,
    drawn_rect_data: RectData,
    styling: ComponentStyling,
    buffer: Rc<RwLock<Vec<u32>>>,
    widht: usize,
    height: usize,
    // default 4
    // bpp: u8,
} 

impl Canvas {
    pub fn new (
    abs_pos: Vertex,
    rel_pos: Vertex,
    styling: Option<ComponentStyling>,
    width: usize,
    height: usize,
    buffer:  Rc<RwLock<Vec<u32>>>, 
    ) -> Self{
    let drawn_rect_data = RectData {
         top_left: Vertex::new(0, 0),
        width: width as u32,
        height: height as u32,
    };
    Self {
        id: None,
        is_dirty: false,
        abs_pos,
        rel_pos,
        drawn_rect_data,
        styling: styling.unwrap_or_default(),
        widht: width,
        height: height,
        buffer: buffer,
        }
    }
     
}

impl Component for Canvas {
    fn draw(&mut self, focus_id: Option<usize>) {
        todo!()
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
        RectData {
            top_left: Vertex { x: 0, y: 0 },
            width: self.widht as u32,
            height: self.height as u32,
        }
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }


    fn rescale_to_container(&mut self, parent: &dyn Container) {}

}


impl Casts for Canvas {}