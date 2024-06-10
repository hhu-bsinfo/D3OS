use alloc::{
    boxed::Box,
    rc::Rc,
    string::{String, ToString},
};
use drawer::drawer::{Drawer, RectData, Vertex};
use graphic::{
    color::Color,
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use spin::Mutex;

use crate::configs::general::DEFAULT_FONT_SCALE;

use super::component::{Component, Interaction};

pub struct Button {
    pub workspace_index: usize,
    pub pos: RectData,
    pub label: Option<Rc<Mutex<String>>>,
    pub on_click: Box<dyn Fn() -> ()>,
}

impl Button {
    pub fn new(
        workspace_index: usize,
        pos: RectData,
        label: Option<Rc<Mutex<String>>>,
        on_click: Box<dyn Fn() -> ()>,
    ) -> Self {
        Self {
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
            width.saturating_sub(DEFAULT_CHAR_WIDTH * (label.chars().count() as u32)) / 2,
            height.saturating_sub(DEFAULT_CHAR_HEIGHT) / 2,
        )
    }
}

impl Component for Button {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        let RectData {
            top_left,
            width,
            height,
        } = self.pos;
        Drawer::draw_rectangle(top_left, top_left.add(width, height), fg_color);
        if let Some(label_mutex) = &self.label {
            let label = &label_mutex.lock();
            let label_pos = self.calc_label_pos(label);

            Drawer::draw_string(
                label.to_string(),
                label_pos,
                fg_color,
                bg_color,
                DEFAULT_FONT_SCALE,
            );
        }
    }

    fn interact(&self, interaction: Interaction) {
        match interaction {
            Interaction::Press => {
                (self.on_click)();
            }
        }
    }
}
