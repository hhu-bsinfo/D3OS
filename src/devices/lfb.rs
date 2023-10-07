use crate::devices::fonts::font_8x8::{CHAR_HEIGHT, CHAR_WIDTH, get_char};
use crate::library::color::Color;

pub struct LFB {
    addr: u64,
    pitch: u32,
    width: u32,
    height: u32,

    pixel_drawer: PixelDrawer
}

impl LFB {
    pub const fn empty() -> Self {
        Self { addr: 0, pitch: 0, width: 0, height: 0, pixel_drawer: draw_pixel_stub }
    }

    pub const fn new(a: u64, p: u32, w: u32, h: u32, b: u8) -> Self {
        let draw_function: PixelDrawer;
        match b {
            15 => {
                draw_function = draw_pixel_15_bit;
            }
            16 => {
                draw_function = draw_pixel_16_bit;
            }
            24 => {
                draw_function = draw_pixel_24_bit;
            }
            32 => {
                draw_function = draw_pixel_32_bit;
            }
            _ => {
                draw_function = draw_pixel_stub;
            }
        };

        Self { addr: a, pitch: p, width: w, height: h, pixel_drawer: draw_function }
    }

    pub fn draw_pixel(&self, x: u32, y: u32, color: &Color) {
        // Check if pixel is outside the framebuffer
        if x >= self.width || y >= self.height {
            return;
        }

        unsafe { (self.pixel_drawer)(self.addr, self.pitch, x, y, color) };
    }

    pub fn draw_char(&self, x: u32, y: u32, fg_color: &Color, bg_color: &Color, c: char) {
        let width_byte = if CHAR_WIDTH % 8 == 0 { CHAR_WIDTH / 8 } else { CHAR_WIDTH / 8 + 1 };
        let bitmap = get_char(c);
        let mut index = 0;

        for offset_y in 0..CHAR_HEIGHT {
            let mut pos_x = x;
            let pos_y = y + offset_y;

            for _xb in 0..width_byte {
                for src in (0..8).rev() {
                    if ((1 << src) & bitmap[index]) != 0 {
                        self.draw_pixel(pos_x, pos_y, fg_color);
                    } else {
                        self.draw_pixel(pos_x, pos_y, bg_color);
                    }

                    pos_x += 1;
                }
            }

            index += 1;
        }
    }

    pub fn clear(&self) {
        let ptr = self.addr as *mut u8;
        unsafe { ptr.write_bytes(0, (self.pitch * self.height) as usize); }
    }

    pub fn scroll_up(&self, lines: u32) {
        let ptr = self.addr as *mut u8;
        unsafe {
            // Move screen buffer upwards by the given amount of lines
            ptr.copy_from(ptr.offset((self.pitch * lines) as isize), (self.pitch * (self.height - lines)) as usize);
            // Clear lower part of the screen
            ptr.offset((self.pitch * (self.height - lines)) as isize).write_bytes(0, (self.pitch * lines) as usize);
        }
    }
}

type PixelDrawer = unsafe fn(addr: u64, pitch: u32, x: u32, y: u32, color: &Color);

fn draw_pixel_stub(addr: u64, pitch: u32, x: u32, y: u32, color: &Color) {
    #![allow(unused_variables)]
    panic!("Using empty LFB!");
}

unsafe fn draw_pixel_15_bit(addr: u64, pitch: u32, x: u32, y: u32, color: &Color) {
    let ptr = addr as *mut u16;
    let index = (x + y * (pitch / 2)) as isize;
    let rgb = color.rgb_15();

    ptr.offset(index).write(rgb as u16) ;
}

unsafe fn draw_pixel_16_bit(addr: u64, pitch: u32, x: u32, y: u32, color: &Color) {
    let ptr = addr as *mut u16;
    let index = (x + y * (pitch / 2)) as isize;
    let rgb = color.rgb_16();

    ptr.offset(index).write(rgb as u16);
}

unsafe fn draw_pixel_24_bit(addr: u64, pitch: u32, x: u32, y: u32, color: &Color) {
    let ptr = addr as *mut u8;
    let index = (x * 3 + y * pitch) as isize;
    let rgb = color.rgb_24();

    ptr.offset(index).write((rgb & 0xff) as u8);
    ptr.offset(index + 1).write(((rgb >> 8) & 0xff) as u8);
    ptr.offset(index + 2).write(((rgb >> 16) & 0xff) as u8);
}

unsafe fn draw_pixel_32_bit(addr: u64, pitch: u32, x: u32, y: u32, color: &Color) {
    let ptr = addr as *mut u32;
    let index = (x + y * (pitch / 4)) as isize;
    let rgb = color.rgb_32();

    ptr.offset(index).write(rgb);
}