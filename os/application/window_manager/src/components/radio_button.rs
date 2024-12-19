// use core::array::from_fn;

// use alloc::{
//     boxed::Box, rc::Rc, string::{String, ToString}, vec::Vec
// };
// use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
// use graphic::{
//     color::{Color, GREY},
//     lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
// };
// use spin::{Mutex, RwLock};

// use crate::{
//     config::{DEFAULT_FG_COLOR, INTERACT_BUTTON}, utils::{scale_circle_to_window, scale_font, scale_rect_to_window}
// };

// use super::component::{Component, ComponentStyling};

// pub const CHECKBOX_BG_COLOR: Color = GREY;
// pub const CHECKBOX_FG_COLOR: Color = DEFAULT_FG_COLOR;

// pub struct RadioButton {
//     pub id: Option<usize>,
//     index: u32,
//     radius: u32,
//     state: bool,
//     abs_center: Vertex,
//     rel_center: Vertex,
//     on_select: Box<dyn Fn(u32) -> ()>,
//     on_change_redraw: Vec<Rc<RwLock<Box<dyn Component>>>>,
//     other_buttons: Vec<Rc<RwLock<RadioButton>>>, 
// }

// impl RadioButton {
//     pub fn new(
//         radius: u32,
//         index: u32,
//         state: bool,
//         abs_center: Vertex,
//         rel_center: Vertex,
//         on_select: Box<dyn Fn(u32) -> ()>,
//         on_change_redraw: Vec<Rc<RwLock<Box<dyn Component>>>>,
//         other_buttons: Vec<Rc<RwLock<RadioButton>>>,
//     ) -> Self {
//         Self {
//             id: None,
//             index,
//             radius,
//             state,
//             rel_center,
//             abs_center,
//             on_select,
//             on_change_redraw,
//             other_buttons,
//         }
//     }

//     pub fn select(&mut self) {
//         self.state = true;
//     }

//     pub fn deselect(&mut self) {
//         self.state = false;
//     }
// }

// impl Component for RadioButton {
//     fn draw(&mut self, is_focused: bool, styling: Option<ComponentStyling>) {
//         // Drawer::draw_circle(self.abs_center, self.radius, fg_color);
//     }

//     fn consume_keyboard_press(&mut self, keyboard_press: char) -> bool {
//         if keyboard_press == INTERACT_BUTTON {
//             // let selected option = 
//             // (*self.on_select)(self.options[0]);
//             return true;
//         }

//         return false;
//     }

//     fn rescale_after_split(&mut self, old_window: RectData, new_window: RectData) {
//         let (abs_center, radius) =  scale_circle_to_window(self.abs_center, self.radius, old_window, DEFAULT_CHAR_HEIGHT);
//         self.abs_center = abs_center;
//         self.radius = radius;
        
//     }

//     fn rescale_after_move(&mut self, new_rect_data: RectData) {
//         // self.abs_center = scale_circle_to_window(self.rel_center, self.radius, new_rect_data, min_radius)
//     }
    
//     fn get_abs_rect_data(&self) -> RectData {
//         let radius = self.radius;

//         RectData {
//             top_left: Vertex::new(
//                 self.abs_center.x - radius,
//                 self.abs_center.y - radius,
//             ),
//             width: radius * 2,
//             height: radius * 2,
//         }
//     }

//     fn get_redraw_components(&self) -> Vec<Rc<RwLock<Box<dyn Component>>>> {
//         Vec::new()
//     }

//     fn disable(&mut self) {
        
//     }

//     fn enable(&mut self) {
        
//     }

//     fn get_id(&self) -> Option<usize> {
//         self.id
//     }

//     fn hide(&mut self) {
        
//     }

//     fn is_dirty(&self) -> bool {
//         false
//     }

//     fn is_disabled(&self) -> bool {
//         false
//     }

//     fn is_hidden(&self) -> bool {
//         false
//     }

//     fn mark_dirty(&mut self) {
        
//     }

//     fn set_id(&mut self, id: usize) {
        
//     }

//     fn show(&mut self) {
        
//     }
// }
