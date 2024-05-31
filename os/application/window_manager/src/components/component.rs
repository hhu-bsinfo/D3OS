use core::any::Any;

use graphic::color::Color;

pub trait Component: Any {
    fn draw(&self, color: Color);

    // Add a method to allow downcasting to immutable ref
    fn as_any(&self) -> &dyn Any;

    // Add a method to allow downcasting to mutable ref
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
