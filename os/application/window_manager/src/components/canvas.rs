// Julius Drodofsky

use alloc::{collections::vec_deque::VecDeque, string::ToString};
use alloc::boxed::Box;
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{bitmap::Bitmap, color::Color};
use super::component::{Casts, Component, ComponentStyling, Interactable};
use alloc::rc::Rc;
use spin::rwlock::RwLock;
use crate::components::container::Container;
use crate::config::INTERACT_BUTTON;
use crate::components::component::*;

pub struct Canvas {
    pub id: Option<usize>,
    is_dirty: bool,
    is_selected: bool,
    abs_rect_data: RectData,
    drawn_rect_data: RectData,
    styling: ComponentStyling,
    buffer: Rc<RwLock<Bitmap>>,
    // get input
    on_change: Rc<Box<dyn Fn(char) -> ()>>,
    
} 

impl Canvas {
    pub fn new (
        styling: Option<ComponentStyling>,
        abs_rect_data: RectData,
        buffer:  Rc<RwLock<Bitmap>>, 
        on_change: Option<Box<dyn Fn(char) -> ()>>,
    ) -> Self{
    let drawn_rect_data = RectData::zero();
    Self {
        id: None,
        is_dirty: true,
        is_selected: true,
        drawn_rect_data: RectData::zero(),
        abs_rect_data,
        styling: styling.unwrap_or_default(),
        buffer: buffer,
        on_change: Rc::new(on_change.unwrap_or_else(|| Box::new(|_| {}))),
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


impl Interactable for Canvas {
    fn consume_keyboard_press(&mut self, keyboard_press: char) -> Option<Box<dyn FnOnce() -> ()>> {
                   // self.input.write().push_back(keyboard_press); 
        //return None;
        let on_change = Rc::clone(&self.on_change);
        return Some(
            Box::new(move || {
                (on_change)(keyboard_press);
            })
        );
    }

    fn consume_mouse_event(&mut self, mouse_event: &crate::mouse_state::MouseEvent) -> Option<Box<dyn FnOnce() -> ()>> {
        if mouse_event.buttons.left.is_pressed()  {
            self.is_selected = !self.is_selected;
        }


        None
    }
}


impl Casts for Canvas {
    fn as_disableable(&self) -> Option<&dyn Disableable> {
        None
    }

    fn as_hideable(&self) -> Option<&dyn Hideable> {
        None
    }

    fn as_focusable(&self) -> Option<&dyn Focusable> {
        None
    }

    fn as_focusable_mut(&mut self) -> Option<&mut dyn Focusable> {
        None
    }

    fn as_interactable(&self) -> Option<&dyn Interactable> {
        Some(self)
    }

    fn as_disableable_mut(&mut self) -> Option<&mut dyn Disableable> {
        None
    }

    fn as_hideable_mut(&mut self) -> Option<&mut dyn Hideable> {
        None
    }

    fn as_interactable_mut(&mut self) -> Option<&mut dyn Interactable> {
        Some(self)
    }

    fn as_clearable_mut(&mut self) -> Option<&mut dyn Clearable> {
        None
    }
}