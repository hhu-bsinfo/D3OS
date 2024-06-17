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

use crate::{
    configs::{components::BUTTON_BG_COLOR, general::DEFAULT_FONT_SCALE},
    utils::scale_rect_to_window,
};

use super::component::{Component, Interaction};

pub struct Button {
    pub workspace_index: usize,
    pub abs_rect_data: RectData,
    pub rel_rect_data: RectData,
    pub label: Option<Rc<Mutex<String>>>,
    pub on_click: Box<dyn Fn() -> ()>,
}

impl Button {
    pub fn new(
        workspace_index: usize,
        abs_rect_data: RectData,
        rel_rect_data: RectData,
        label: Option<Rc<Mutex<String>>>,
        on_click: Box<dyn Fn() -> ()>,
    ) -> Self {
        Self {
            workspace_index,
            abs_rect_data,
            rel_rect_data,
            label,
            on_click,
        }
    }

    fn calc_label_pos(&self, label: &str) -> Vertex {
        let RectData {
            top_left,
            width,
            height,
        } = self.abs_rect_data;

        top_left.add(
            width.saturating_sub(DEFAULT_CHAR_WIDTH * (label.chars().count() as u32)) / 2,
            height.saturating_sub(DEFAULT_CHAR_HEIGHT) / 2,
        )
    }
}

impl Component for Button {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        Drawer::draw_filled_rectangle(self.abs_rect_data, BUTTON_BG_COLOR, Some(fg_color));
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

    fn rescale_in_place(&mut self, old_window: RectData, new_window: RectData) {
        self.abs_rect_data = self.abs_rect_data.scale(&old_window, &new_window);
    }

    fn rescale_after_move(&mut self, new_window_rect_data: RectData) {
        self.abs_rect_data = scale_rect_to_window(self.rel_rect_data, new_window_rect_data);
    }
}
