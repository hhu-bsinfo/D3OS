use drawer::drawer::RectData;
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
    fn rescale(&mut self, old_window: &RectData, new_window: &RectData);
}
