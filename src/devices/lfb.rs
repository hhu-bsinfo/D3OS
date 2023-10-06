use crate::devices::fonts::font_8x8::{CHAR_HEIGHT, CHAR_WIDTH, get_char};

pub struct LFB {
    addr: u64,
    pitch: u32,
    width: u32,
    height: u32,
    bpp: u8,
}

impl LFB {
    pub const fn empty() -> Self {
        Self { addr: 0, pitch: 0, width: 0, height: 0, bpp: 0 }
    }

    pub const fn new(a: u64, p: u32, w: u32, h: u32, b: u8) -> Self {
        Self { addr: a, pitch: p, width: w, height: h, bpp: b }
    }

    pub fn draw_pixel(&self, x: u32, y: u32, col: u32) {
        // Check if pixel is outside the framebuffer
        if x >= self.width || y >= self.height {
            return;
        }

        // Calculate pixel address and write color value
        match self.bpp {
            15 | 16 => {
                let ptr = self.addr as *mut u16;
                let index = (x + y * (self.pitch / 2)) as isize;

                unsafe { ptr.offset(index).write(col as u16) };
            },
            24 => {
                let ptr = self.addr as *mut u8;
                let index = (x * 3 + y * self.pitch) as isize;

                unsafe {
                    ptr.offset(index).write((col & 0xff) as u8);
                    ptr.offset(index + 1).write(((col >> 8) & 0xff) as u8);
                    ptr.offset(index + 2).write(((col >> 16) & 0xff) as u8);
                }
            },
            32 => {
                let ptr = self.addr as *mut u32;
                let index = (x + y * (self.pitch / 4)) as isize;

                unsafe { ptr.offset(index).write(col) };
            },
            _ => {
                panic!("LFB: Unsupported bit depth {}!", self.bpp);
            }
        }
    }

    pub fn draw_char(&self, x: u32, y: u32, col: u32, c: char) {
        let width_byte = if CHAR_WIDTH % 8 == 0 { CHAR_WIDTH / 8 } else { CHAR_WIDTH / 8 + 1 };
        let bitmap = get_char(c);
        let mut index = 0;

        for offset_y in 0..CHAR_HEIGHT {
            let mut pos_x = x;
            let pos_y = y + offset_y;

            for _xb in 0..width_byte {
                for src in (0..8).rev() {
                    if ((1 << src) & bitmap[index]) != 0 {
                        self.draw_pixel(pos_x, pos_y, col);
                    }

                    pos_x += 1;
                }
            }

            index += 1;
        }
    }

    pub unsafe fn scroll_up(&self, lines: u32) {
        // Move screen buffer upwards by the given amount of lines
        let ptr = self.addr as *mut u8;
        ptr.copy_from(ptr.offset((self.pitch * lines) as isize), (self.pitch * (self.height - lines)) as usize);

        // Clear lower part of the screen
        ptr.offset((self.pitch * (self.height - lines)) as isize).write_bytes(0, (self.pitch * lines) as usize);
    }
}