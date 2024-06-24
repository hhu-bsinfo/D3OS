use alloc::{
    boxed::Box,
    rc::Rc,
    string::{String, ToString},
};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{
    color::{Color, GREY},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use spin::Mutex;

use crate::{
    config::INTERACT_BUTTON,
    utils::{scale_font, scale_rect_to_window},
};

use super::component::Component;

pub const BUTTON_BG_COLOR: Color = GREY;

pub struct Button {
    abs_rect_data: RectData,
    rel_rect_data: RectData,
    label: Option<Rc<Mutex<String>>>,
    rel_font_size: usize,
    font_scale: (u32, u32),
    on_click: Box<dyn Fn() -> ()>,
}

impl Button {
    pub fn new(
        abs_rect_data: RectData,
        rel_rect_data: RectData,
        label: Option<Rc<Mutex<String>>>,
        rel_font_size: usize,
        font_scale: (u32, u32),
        on_click: Box<dyn Fn() -> ()>,
    ) -> Self {
        Self {
            abs_rect_data,
            rel_rect_data,
            rel_font_size,
            font_scale,
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
            width.saturating_sub(
                DEFAULT_CHAR_WIDTH * self.font_scale.0 * (label.chars().count() as u32),
            ) / 2,
            height.saturating_sub(DEFAULT_CHAR_HEIGHT * self.font_scale.1) / 2,
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
                self.font_scale,
            );
        }
    }

    fn consume_keyboard_press(&mut self, keyboard_press: char) -> bool {
        if keyboard_press == INTERACT_BUTTON {
            (self.on_click)();
            return true;
        }

        return false;
    }

    fn rescale_after_split(&mut self, old_window: RectData, new_window: RectData) {
        self.abs_rect_data.top_left = self
            .abs_rect_data
            .top_left
            .move_to_new_rect(&old_window, &new_window);

        let min_dim = match &self.label {
            Some(label) => Some((
                label.lock().len() as u32 * DEFAULT_CHAR_WIDTH * self.font_scale.0,
                DEFAULT_CHAR_HEIGHT * self.font_scale.1,
            )),
            None => None,
        };

        self.abs_rect_data = self
            .abs_rect_data
            .scale_dimensions(&old_window, &new_window, min_dim);

        self.font_scale = scale_font(&self.font_scale, &old_window, &new_window);
    }

    fn rescale_after_move(&mut self, new_rect_data: RectData) {
        let min_width = match &self.label {
            Some(label) => label.lock().len() as u32 * DEFAULT_CHAR_WIDTH * self.font_scale.0,
            None => 0,
        };
        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_rect_data,
            (min_width, DEFAULT_CHAR_HEIGHT * self.font_scale.1),
        );

        self.font_scale = scale_font(
            &(self.rel_font_size as u32, self.rel_font_size as u32),
            &self.rel_rect_data,
            &self.abs_rect_data,
        );
    }
}
