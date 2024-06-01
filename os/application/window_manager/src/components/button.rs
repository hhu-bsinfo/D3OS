use alloc::boxed::Box;
use drawer::drawer::{Drawer, RectData};
use graphic::lfb::{CHAR_HEIGHT, CHAR_WIDTH};

use super::component::Component;

pub struct Button {
    pub comp_id: usize,
    pub workspace_index: usize,
    pub pos: RectData,
    pub label: char,
    pub on_click: Box<dyn Fn() -> ()>,
}

impl Button {
    pub fn new(
        comp_id: usize,
        workspace_index: usize,
        pos: RectData,
        label: char,
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
        // Assuming the label contains only one char (for now)
        let label_pos = top_left.add(
            width.saturating_sub(CHAR_WIDTH) / 2,
            height.saturating_sub(CHAR_HEIGHT) / 2,
        );

        Drawer::draw_char(self.label, label_pos, color);
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn core::any::Any {
        self
    }
}
