use font8x8::{BASIC_FONTS, BLOCK_FONTS, BOX_FONTS, GREEK_FONTS, HIRAGANA_FONTS, LATIN_FONTS, MISC_FONTS, SGA_FONTS, UnicodeFonts};
use crate::library::graphic::color::Color;

pub struct LFB {
    buffer: *mut u8,
    pitch: u32,
    width: u32,
    height: u32,
    bpp: u8,

    pixel_drawer: PixelDrawer
}

unsafe impl Send for LFB {}
unsafe impl Sync for LFB {}

pub const CHAR_HEIGHT: u32 = 16;
pub const CHAR_WIDTH: u32 = 8;

impl LFB {
    pub const fn new(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        let pixel_drawer: PixelDrawer = match bpp {
            15 => {
                draw_pixel_15_bit
            }
            16 => {
                draw_pixel_16_bit
            }
            24 => {
                draw_pixel_24_bit
            }
            32 => {
                draw_pixel_32_bit
            }
            _ => {
                draw_pixel_stub
            }
        };

        Self { buffer, pitch, width, height, bpp, pixel_drawer }
    }

    pub const fn buffer(&self) -> *mut u8 {
        self.buffer
    }

    pub const fn pitch(&self) -> u32 {
        self.pitch
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn bpp(&self) -> u8 {
        self.bpp
    }

    pub fn draw_pixel(&self, x: u32, y: u32, color: &Color) {
        // Check if pixel is outside the framebuffer
        if x >= self.width || y >= self.height {
            return;
        }

        unsafe { (self.pixel_drawer)(self.buffer, self.pitch, x, y, color) };
    }

    pub fn fill_rect(&self, x: u32, y: u32, width: u32, height: u32, color: &Color) {
        let end_x = x + width;
        let end_y = y + height;

        for i in y..end_y {
            for j in x..end_x {
                self.draw_pixel(j, i, color);
            }
        }
    }

    pub fn draw_char(&self, x: u32, y: u32, fg_color: &Color, bg_color: &Color, c: char) -> bool {
        let mut glyph = BASIC_FONTS.get(c);
        if glyph.is_none() {
            glyph = LATIN_FONTS.get(c);
            if glyph.is_none() {
                glyph = BLOCK_FONTS.get(c);
                if glyph.is_none() {
                    glyph = BOX_FONTS.get(c);
                    if glyph.is_none() {
                        glyph = MISC_FONTS.get(c);
                        if glyph.is_none() {
                            glyph = GREEK_FONTS.get(c);
                            if glyph.is_none() {
                                glyph = HIRAGANA_FONTS.get(c);
                                if glyph.is_none() {
                                    glyph = SGA_FONTS.get(c);
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(bitmap) = glyph {
            let mut x_offset = 0;
            let mut y_offset = 0;

            for row in &bitmap {
                for col in 0..8 {
                    let color = match *row & 1 << col {
                        0 => bg_color,
                        _ => fg_color
                    };

                    self.draw_pixel(x + x_offset, y + y_offset, color);
                    self.draw_pixel(x + x_offset, y + y_offset + 1, color);
                    x_offset += 1;
                }

                x_offset = 0;
                y_offset += 2;
            }

            return true;
        }

        return false;
    }

    pub fn clear(&self) {
        unsafe { self.buffer.write_bytes(0, (self.pitch * self.height) as usize); }
    }

    pub fn scroll_up(&self, lines: u32) {
        unsafe {
            // Move screen buffer upwards by the given amount of lines
            self.buffer.copy_from(self.buffer.offset((self.pitch * lines) as isize), (self.pitch * (self.height - lines)) as usize);
            // Clear lower part of the screen
            self.buffer.offset((self.pitch * (self.height - lines)) as isize).write_bytes(0, (self.pitch * lines) as usize);
        }
    }
}

type PixelDrawer = unsafe fn(addr: *mut u8, pitch: u32, x: u32, y: u32, color: &Color);

fn draw_pixel_stub(addr: *mut u8, pitch: u32, x: u32, y: u32, color: &Color) {
    #![allow(unused_variables)]
    panic!("Using empty LFB!");
}

unsafe fn draw_pixel_15_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: &Color) {
    let index = (x + y * (pitch / 2)) as isize;
    let rgb = color.rgb_15();

    (addr as *mut u16).offset(index).write(rgb) ;
}

unsafe fn draw_pixel_16_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: &Color) {
    let index = (x + y * (pitch / 2)) as isize;
    let rgb = color.rgb_16();

    (addr as *mut u16).offset(index).write(rgb);
}

unsafe fn draw_pixel_24_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: &Color) {
    let index = (x * 3 + y * pitch) as isize;
    let rgb = color.rgb_24();

    addr.offset(index).write((rgb & 0xff) as u8);
    addr.offset(index + 1).write(((rgb >> 8) & 0xff) as u8);
    addr.offset(index + 2).write(((rgb >> 16) & 0xff) as u8);
}

unsafe fn draw_pixel_32_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: &Color) {
    let index = (x + y * (pitch / 4)) as isize;
    let rgb = color.rgb_32();

    (addr as *mut u32).offset(index).write(rgb);
}