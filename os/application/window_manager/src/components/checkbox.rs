use alloc::{
    boxed::Box, rc::Rc, string::{String, ToString}, vec::Vec
};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{
    color::{Color, GREY},
    lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH},
};
use spin::{Mutex, RwLock};

use crate::{
    config::{DEFAULT_FG_COLOR, INTERACT_BUTTON}, utils::{scale_font, scale_rect_to_window}
};

use super::component::Component;

pub const CHECKBOX_BG_COLOR: Color = GREY;
pub const CHECKBOX_FG_COLOR: Color = DEFAULT_FG_COLOR;

pub struct Checkbox {
    abs_rect_data: RectData,
    rel_rect_data: RectData,
    // label: Option<Rc<Mutex<String>>>,
    // rel_font_size: usize,
    // font_scale: (u32, u32),
    state: bool,
    on_checked: Box<dyn Fn() -> ()>,
    on_unchecked: Box<dyn Fn() -> ()>,
    on_change_redraw: Vec<Rc<RwLock<Box<dyn Component>>>>,
}

impl Checkbox {
    pub fn new(
        abs_rect_data: RectData,
        rel_rect_data: RectData,
        // label: Option<Rc<Mutex<String>>>,
        // rel_font_size: usize,
        // font_scale: (u32, u32),
        state: bool,
        on_checked: Box<dyn Fn() -> ()>,
        on_unchecked: Box<dyn Fn() -> ()>,
        on_change_redraw: Vec<Rc<RwLock<Box<dyn Component>>>>,
    ) -> Self {
        Self {
            abs_rect_data,
            rel_rect_data,
            // rel_font_size,
            // label,
            // font_scale,
            state,
            on_checked,
            on_unchecked,
            on_change_redraw,
        }
    }

    // fn calc_label_pos(&self, label: &str) -> Vertex {
    //     let RectData {
    //         top_left,
    //         width,
    //         height,
    //     } = self.abs_rect_data;

    //     top_left.add(
    //         width.saturating_sub(
    //             DEFAULT_CHAR_WIDTH * self.font_scale.0 * (label.chars().count() as u32),
    //         ) / 2,
    //         height + DEFAULT_CHAR_HEIGHT * self.font_scale.1,
    //     )
    // }
}

impl Component for Checkbox {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        Drawer::draw_filled_rectangle(self.abs_rect_data, CHECKBOX_BG_COLOR, Some(fg_color));

        if self.state {
            let RectData { top_left, width, height } = self.abs_rect_data;
            let bottom_right = top_left.add(width, height);
            let top_right = top_left.add(width, 0);
            let bottom_left = top_left.add(0, height);

            Drawer::draw_line(top_left, bottom_right, CHECKBOX_FG_COLOR);
            Drawer::draw_line(top_right, bottom_left, CHECKBOX_FG_COLOR);
        }
        
        // if let Some(label_mutex) = &self.label {
        //     let label = &label_mutex.lock();
        //     let label_pos = self.calc_label_pos(label);

        //     Drawer::draw_string(
        //         label.to_string(),
        //         label_pos,
        //         fg_color,
        //         bg_color,
        //         self.font_scale,
        //     );
        // }
    }

    fn consume_keyboard_press(&mut self, keyboard_press: char) -> bool {
        if keyboard_press == INTERACT_BUTTON {
            self.state = !self.state;

            if self.state {
                (self.on_checked)();
            } else {
                (self.on_unchecked)();
            }
            
            return true;
        }

        return false;
    }

    fn rescale_after_split(&mut self, old_window: RectData, new_window: RectData) {
        self.abs_rect_data.top_left = self
            .abs_rect_data
            .top_left
            .move_to_new_rect(&old_window, &new_window);

        let min_dim = Some((
            DEFAULT_CHAR_HEIGHT,
            DEFAULT_CHAR_HEIGHT,
        ));

        self.abs_rect_data = self
            .abs_rect_data
            .scale_dimensions(&old_window, &new_window, min_dim);
    }

    fn rescale_after_move(&mut self, new_rect_data: RectData) {
        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_rect_data,
            (DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_HEIGHT),
        );
    }
    
    fn get_abs_rect_data(&self) -> RectData {
        self.abs_rect_data
    }

    fn get_redraw_components(&self) -> Vec<Rc<RwLock<Box<dyn Component>>>> {
        self.on_change_redraw.clone()
    }
}
