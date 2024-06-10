use graphic::color::Color;

pub enum Interaction {
    Press,
}

/// Everything size related is specified in relation to the window the component is contained in
pub trait Component {
    fn draw(&self, color: Color);

    /// Dictates if/how a component should react to different interactions
    fn interact(&self, interaction: Interaction);
}
