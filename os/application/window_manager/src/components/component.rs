use drawer::rect_data::RectData;
use graphic::color::Color;

pub enum Interaction {
    Press,
}

/// Everything size related is specified in relation to the window the component is contained in
pub trait Component {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>);

    /// Dictates if/how a component should react to different interactions
    fn interact(&self, interaction: Interaction);

    /// Defines how rescaling the component-geometry works after the containing window has been resized
    fn rescale_after_split(&mut self, old_rect_data: RectData, new_rect_data: RectData);

    fn rescale_after_move(&mut self, new_window_rect_data: RectData);
}
