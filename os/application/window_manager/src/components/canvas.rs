// Julius Drodofsky
use terminal::{print, DecodedKey};

use alloc::{collections::vec_deque::VecDeque, string::ToString};
use alloc::boxed::Box;
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{bitmap::Bitmap, color::Color};
use terminal::println;
use super::component::{Casts, Component, ComponentStyling, Interactable};
use alloc::rc::Rc;
use spin::rwlock::RwLock;
use crate::components::container::Container;
use crate::config::INTERACT_BUTTON;
use crate::components::component::*;
use crate::WindowManager;

pub struct Canvas {
    pub id: usize,
    is_dirty: bool,
    is_selected: bool,
    abs_rect_data: RectData,
    drawn_rect_data: RectData,
    styling: ComponentStyling,
    buffer: Rc<RwLock<Bitmap>>,
    // function to get user input
    input: Rc<Box<dyn Fn(DecodedKey) -> ()>>,
    
} 

impl Canvas {
    pub fn new (
        styling: Option<ComponentStyling>,
        abs_rect_data: RectData,
        buffer:  Rc<RwLock<Bitmap>>, 
        input: Option<Box<dyn Fn(DecodedKey) -> ()>>,
    ) -> Self{
    let drawn_rect_data = RectData::zero();
    Self {
        id: WindowManager::generate_id(),
        is_dirty: true,
        is_selected: false,
        drawn_rect_data: RectData::zero(),
        abs_rect_data,
        styling: styling.unwrap_or_default(),
        buffer: buffer,
        input: Rc::new(input.unwrap_or_else(|| Box::new(|_| {}))),
        }
    }
     
}

impl Component for Canvas {
    fn draw(&mut self, focus_id: Option<usize>) {
        if !self.is_dirty{
            return;
        }
        let is_focused = focus_id == Some(self.id);
        let styling = &self.styling;

        let border_color = if self.is_selected {
            styling.selected_border_color
        }  else if is_focused {
            styling.focused_border_color
        } else {
            styling.border_color
        };
        // only small corner to save render time
        let border_data = RectData{ top_left: Vertex { x: self.abs_rect_data.top_left.x-2, y: self.abs_rect_data.top_left.y-2 }, width: 20, height: 20 };
        Drawer::draw_filled_rectangle(border_data, Color { red: 0, green: 0, blue: 0, alpha: 0 }, Some(border_color));
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

    fn get_id(&self) -> usize {
        self.id
    }

    fn get_abs_rect_data(&self) -> RectData {
       self.abs_rect_data 
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }


    fn rescale_to_container(&mut self, parent: &dyn Container) {}

}

impl Focusable for Canvas {
    fn can_unfocus(&self) -> bool {
        !self.is_selected
    }

    fn focus(&mut self) {
        self.mark_dirty();
    }

    fn unfocus(&mut self) {
        self.mark_dirty();
    }
}

impl Interactable for Canvas {
    fn consume_keyboard_press(&mut self, keyboard_press: DecodedKey) -> Option<Box<dyn FnOnce() -> ()>> {
        let input = Rc::clone(&self.input);
        return Some(
            Box::new(move || {
                (input)(keyboard_press);
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
        Some(self)
    }

    fn as_focusable_mut(&mut self) -> Option<&mut dyn Focusable> {
        Some(self)
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