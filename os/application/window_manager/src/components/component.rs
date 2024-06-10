use core::any::Any;

use graphic::color::Color;

pub enum Interaction {
    Press,
}

pub trait Component: Any {
    fn draw(&self, color: Color);

    /// Dictates if/how a component should react to different interactions
    fn interact(&self, interaction: Interaction);

    /// Allows downcasting to immutable ref
    fn as_any(&self) -> &dyn Any;

    /// Allows downcasting to mutable ref
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
