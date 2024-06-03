use alloc::{
    boxed::Box,
    rc::Rc,
    string::{String, ToString},
};
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::lfb::{CHAR_HEIGHT, CHAR_WIDTH};
use spin::Mutex;

use super::component::Component;

pub struct Button {
    pub comp_id: usize,
    pub workspace_index: usize,
    pub pos: RectData,
    // TODO: Consider using Arc instead of Rc, just in case
    pub label: Option<Rc<Mutex<String>>>,
    pub on_click: Box<dyn Fn() -> ()>,
}

impl Button {
    pub fn new(
        comp_id: usize,
        workspace_index: usize,
        pos: RectData,
        label: Option<Rc<Mutex<String>>>,
        on_click: Box<dyn Fn() -> ()>,
    ) -> Self {
        Self {
            comp_id,
            workspace_index,
            pos,
            label,
            on_click,
        }
    }

    fn calc_label_pos(&self, label: &str) -> Vertex {
        let RectData {
            top_left,
            width,
            height,
        } = self.pos;

        top_left.add(
            width.saturating_sub(CHAR_WIDTH * (label.chars().count() as u32)) / 2,
            height.saturating_sub(CHAR_HEIGHT) / 2,
        )
    }
}

impl Component for Button {
    fn id(&self) -> usize {
        self.comp_id
    }

    fn workspace_index(&self) -> usize {
        self.workspace_index
    }

    fn draw(&self, color: graphic::color::Color) {
        let RectData {
            top_left,
            width,
            height,
        } = self.pos;
        Drawer::draw_rectangle(top_left, top_left.add(width, height), color);
        if let Some(label_mutex) = &self.label {
            let label = &label_mutex.lock();
            let label_pos = self.calc_label_pos(label);

            Drawer::draw_string(label.to_string(), label_pos, color);
        }
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn core::any::Any {
        self
    }
}
