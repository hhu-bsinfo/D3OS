// use alloc::{boxed::Box, rc::Rc, vec::Vec};
// use drawer::vertex::Vertex;
// use spin::rwlock::RwLock;

// use super::{component::{Component, ComponentStyling}, radio_button::RadioButton};

// pub struct RadioButtonGroup {
//     pub id: Option<usize>,
//     num_buttons: u32,
//     selected_index: u32,
//     buttons: Vec<Rc<RwLock<RadioButton>>>,
// }

// impl RadioButtonGroup {
//     pub fn new(num_buttons: u32, selected_index: u32, buttons: Vec<Rc<RwLock<RadioButton>>>) -> Self {
//         Self {
//             id: None,
//             selected_index,
//             num_buttons,
//             buttons,
//         }
//     }

//     pub fn on_select(&self, index: usize) {
//         // self.selected_index = index;
//     }
// }

// // impl Component for RadioButtonGroup {
// //     fn draw(&mut self, _is_focused: bool, _styling: Option<ComponentStyling>) {
// //         for button in self.buttons.iter_mut() {
// //             button.draw(false, None);
// //         }
// //     }

// //     fn consume_keyboard_press(&mut self, _keyboard_press: char) -> bool {
// //         return false;
// //     }

// //     fn rescale_after_move(&mut self, _new_rect_data: drawer::rect_data::RectData) {
// //         //
// //     }

// //     fn rescale_after_split(&mut self, _old_rect_data: drawer::rect_data::RectData, _new_rect_data: drawer::rect_data::RectData) {
// //         //
// //     }

// //     fn disable(&mut self) {
        
// //     }

// //     fn enable(&mut self) {
        
// //     }

// //     fn get_abs_rect_data(&self) -> drawer::rect_data::RectData {
// //         drawer::rect_data::RectData {
// //             top_left: Vertex::new(0, 0),
// //             width: 0,
// //             height: 0,
// //         }
// //     }

// //     fn get_id(&self) -> Option<usize> {
// //         None
// //     }

// //     fn hide(&mut self) {
        
// //     }

// //     fn show(&mut self) {
        
// //     }

// //     fn is_dirty(&self) -> bool {
// //         false
// //     }

// //     fn is_hidden(&self) -> bool {
// //         false
// //     }

// //     fn is_disabled(&self) -> bool {
// //         false
// //     }

// //     fn mark_dirty(&mut self) {
        
// //     }

// //     fn get_redraw_components(&self) -> Vec<alloc::rc::Rc<spin::RwLock<alloc::boxed::Box<dyn Component>>>> {
// //         Vec::new()
// //     }

// //     fn set_id(&mut self, id: usize) {
        
// //     }
// // }