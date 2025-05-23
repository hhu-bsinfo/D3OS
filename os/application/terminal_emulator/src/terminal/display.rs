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
    pub(crate) lfb: BufferedLFB,
    pub(crate) char_buffer: Vec<Character>,
}

impl DisplayState {
    pub fn new(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
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

        lfb.lfb().clear();
        lfb.flush();

        Self {
            size,
            lfb,
            char_buffer,
        }
    }
}
