use crate::signal::ComponentRef;

use super::component::Component;

pub mod basic_container;

pub trait Container: Component {
    fn add_child(&mut self, child: ComponentRef);
}
