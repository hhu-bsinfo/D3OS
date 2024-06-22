use drawer::rect_data::RectData;
use graphic::color::Color;

/// Everything size related is specified in relation to the window the component is contained in
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

    fn rescale_after_move(&mut self, new_window_rect_data: RectData);
}
