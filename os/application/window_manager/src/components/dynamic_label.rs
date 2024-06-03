use core::{any::Any, ops::Deref};

use alloc::{boxed::Box, rc::Rc, string::String};
use drawer::drawer::{Drawer, Vertex};
use graphic::color::Color;
use spin::RwLock;

use super::component::Component;

/// Dynamic Labels are characterized by their text being modifiable, unlike [`Label`](`crate::components::label::Label`)
/// These do not work, until we have support for creating threads with the type `Fn() -> ()`
pub struct DynamicLabel {
    pub id: usize,
    pub workspace_index: usize,
    pub pos: Vertex,
    pub text: Rc<RwLock<String>>,
    pub on_create: Option<Box<dyn Fn() -> ()>>,
}

impl DynamicLabel {
    pub fn new(
        id: usize,
        workspace_index: usize,
        pos: Vertex,
        text: Rc<RwLock<String>>,
        on_create: Option<Box<dyn Fn() -> ()>>,
    ) -> Self {
        Self {
            id,
            workspace_index,
            pos,
            text,
            on_create,
        }
    }

    pub fn call_on_create(&self) {
        if let Some(fun) = &self.on_create {
            fun();
        }
    }
}

impl Component for DynamicLabel {
    fn id(&self) -> usize {
        self.id
    }

    fn workspace_index(&self) -> usize {
        self.workspace_index
    }

    fn draw(&self, color: Color) {
        let text = self.text.read();
        Drawer::draw_string(text.deref().clone(), self.pos, color);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
