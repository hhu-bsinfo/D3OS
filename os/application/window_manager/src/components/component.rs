use alloc::{boxed::Box, rc::Rc, vec::Vec};
use drawer::rect_data::RectData;
use graphic::color::Color;
use spin::RwLock;

/**
Any size-relations with the words "rel" or "relative" in them refer to the size inside the window
as if the window was occupying the full screen
*/
pub trait Component {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>);

    /**
    Dictates if/how a component should react to keyboard-button presses.
    If it returns false, the button-press will be consumed by the
    window-manager instead/too.
    */
    fn consume_keyboard_press(&mut self, keyboard_press: char) -> bool;

    /// Defines how rescaling the component-geometry works after the containing window has been resized
    fn rescale_after_split(&mut self, old_rect_data: RectData, new_rect_data: RectData);

    fn rescale_after_move(&mut self, new_rect_data: RectData);

    fn get_abs_rect_data(&self) -> RectData;

    fn get_state_dependencies(&self) -> Vec<Rc<RwLock<Box<dyn Component>>>>;
}