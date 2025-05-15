use alloc::vec::Vec;
use graphic::{
    buffered_lfb::BufferedLFB,
    color::{self, Color},
    lfb::{self, LFB},
};

#[derive(Copy, Clone)]
pub struct Character {
    pub value: char,
    pub fg_color: Color,
    pub bg_color: Color,
}

pub struct DisplayState {
    pub(crate) size: (u16, u16),
    lfb: BufferedLFB,
    pub(crate) char_buffer: Vec<Character>,
    visible: bool,
}

impl DisplayState {
    pub fn new(
        buffer: *mut u8,
        pitch: u32,
        width: u32,
        height: u32,
        bpp: u8,
        visible: bool,
    ) -> Self {
        let raw_lfb = LFB::new(buffer, pitch, width, height, bpp);
        let mut lfb = BufferedLFB::new(raw_lfb);
        let size = (
            (width / lfb::DEFAULT_CHAR_WIDTH) as u16,
            (height / lfb::DEFAULT_CHAR_HEIGHT) as u16,
        );

        let mut char_buffer =
            Vec::with_capacity(size.0 as usize * size.1 as usize * size_of::<Character>());
        for _ in 0..char_buffer.capacity() {
            char_buffer.push(Character {
                value: ' ',
                fg_color: color::WHITE,
                bg_color: color::BLACK,
            });
        }

        if visible {
            lfb.lfb().clear();
            lfb.flush();
        }

        Self {
            size,
            lfb,
            char_buffer,
            visible,
        }
    }

    pub fn scroll_up(&mut self, lines: u32) {
        match self.visible {
            true => self.lfb.lfb().scroll_up(lines),
            false => return,
        }
    }

    pub fn draw_pixel(&mut self, x: u32, y: u32, color: Color) {
        match self.visible {
            true => self.lfb.lfb().draw_pixel(x, y, color),
            false => return,
        }
    }

    pub fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        match self.visible {
            true => self.lfb.lfb().fill_rect(x, y, width, height, color),
            false => return,
        }
    }

    pub fn draw_char(&mut self, x: u32, y: u32, fg_color: Color, bg_color: Color, c: char) -> u32 {
        match self.visible {
            true => self.lfb.lfb().draw_char(x, y, fg_color, bg_color, c),
            false => return 0,
        }
    }

    pub fn draw_direct_char(
        &mut self,
        x: u32,
        y: u32,
        fg_color: Color,
        bg_color: Color,
        c: char,
    ) -> u32 {
        match self.visible {
            true => self.lfb.direct_lfb().draw_char(x, y, fg_color, bg_color, c),
            false => return 0,
        }
    }

    pub fn draw_string(&mut self, x: u32, y: u32, fg_color: Color, bg_color: Color, string: &str) {
        match self.visible {
            true => self.lfb.lfb().draw_string(x, y, fg_color, bg_color, string),
            false => return,
        }
    }

    pub fn flush(&mut self) {
        match self.visible {
            true => self.lfb.flush(),
            false => return,
        }
    }

    pub fn flush_lines(&mut self, start: u32, count: u32) {
        match self.visible {
            true => self.lfb.flush_lines(start, count),
            false => return,
        }
    }

    pub fn toggle_visibility(&mut self) {
        match self.visible {
            true => {
                self.lfb.direct_lfb().clear();
                self.visible = false;
            }
            false => {
                self.lfb.direct_lfb().clear();
                self.lfb.flush();
                self.visible = true;
            }
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }
}
