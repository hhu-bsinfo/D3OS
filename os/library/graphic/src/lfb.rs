use unifont::get_glyph;
use crate::color::Color;

pub struct LFB {
    buffer: *mut u8,
    pitch: u32,
    width: u32,
    height: u32,
    bpp: u8,

    pixel_drawer: PixelDrawer,
}

unsafe impl Send for LFB {}
unsafe impl Sync for LFB {}

pub const DEFAULT_CHAR_WIDTH: u32 = 8;
pub const DEFAULT_CHAR_HEIGHT: u32 = 16;

impl LFB {
    pub const fn new(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        let pixel_drawer: PixelDrawer = match bpp {
            15 => draw_pixel_15_bit,
            16 => draw_pixel_16_bit,
            24 => draw_pixel_24_bit,
            32 => draw_pixel_32_bit,
            _ => draw_pixel_stub,
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

    pub fn draw_pixel(&self, x: u32, y: u32, color: Color) {
        // Check if pixel is outside the framebuffer
        if x >= self.width || y >= self.height {
            return;
        }

        // Do not draw pixels with alpha = 0
        if color.alpha == 0 {
            return;
        }

        // Blend if necessary and draw pixel
        if color.alpha < 255 {
            unsafe { (self.pixel_drawer)(self.buffer, self.pitch, x, y, self.read_pixel(x, y).blend(color)) };
        } else {
            unsafe { (self.pixel_drawer)(self.buffer, self.pitch, x, y, color) };
        }
    }

    pub fn read_pixel(&self, x: u32, y: u32) -> Color {
        if x > self.width - 1 || y > self.height - 1 {
            panic!("LinearFrameBuffer: Trying to read a pixel out of bounds!");
        }

        let bpp = if self.bpp == 15 { 16 } else { self.bpp() };

        unsafe {
            let ptr = self.buffer.offset(((x * (bpp / 8) as u32) + y * self.pitch) as isize) as *const u32;
            Color::from_rgb(ptr.read(), self.bpp)
        }
    }

    pub fn fill_rect(&self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        let end_x = x + width;
        let end_y = y + height;

        for i in y..end_y {
            for j in x..end_x {
                self.draw_pixel(j, i, color);
            }
        }
    }

    pub fn draw_char(&self, x: u32, y: u32, fg_color: Color, bg_color: Color, c: char) -> u32 {
        self.draw_char_scaled(x, y, 1, 1, fg_color, bg_color, c)
    }

    pub fn draw_char_scaled(&self, x: u32, y: u32, x_scale: u32, y_scale: u32, fg_color: Color, bg_color: Color, c: char) -> u32 {
        return match get_glyph(c) {
            Some(glyph) => {
                let mut x_offset = 0;
                let mut y_offset = 0;

                for row in 0..DEFAULT_CHAR_HEIGHT {
                    for col in 0..glyph.get_width() as u32 {
                        let color = match glyph.get_pixel(col as usize, row as usize) {
                            true => fg_color,
                            false => bg_color
                        };

                        for i in 0..x_scale {
                            for j in 0..y_scale {
                                self.draw_pixel(x + x_offset + i, y + y_offset + j, color);
                            }
                        }

                        x_offset += x_scale;
                    }

                    x_offset = 0;
                    y_offset += y_scale;
                }

                glyph.get_width() as u32
            },
            None => 0
        }
    }

    pub fn draw_string(&self, x: u32, y: u32, fg_color: Color, bg_color: Color, string: &str) {
        self.draw_string_scaled(x, y, 1, 1, fg_color, bg_color, string);
    }

    pub fn draw_string_scaled(&self, x: u32, y: u32, x_scale: u32, y_scale: u32, fg_color: Color, bg_color: Color, string: &str) {
        for c in string.chars().enumerate() {
            self.draw_char_scaled(x + (c.0 as u32 * (8 * x_scale)), y, x_scale, y_scale, fg_color, bg_color, c.1);
        }
    }

    pub fn clear(&self) {
        unsafe {
            self.buffer.write_bytes(0, (self.pitch * self.height) as usize);
        }
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

type PixelDrawer = unsafe fn(addr: *mut u8, pitch: u32, x: u32, y: u32, color: Color);

fn draw_pixel_stub(_addr: *mut u8, _pitch: u32, _x: u32, _y: u32, _color: Color) {
    panic!("Using empty LFB!");
}

unsafe fn draw_pixel_15_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: Color) {
    let index = (x + y * (pitch / 2)) as isize;
    let rgb = color.rgb_15();

    (addr as *mut u16).offset(index).write(rgb);
}

unsafe fn draw_pixel_16_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: Color) {
    let index = (x + y * (pitch / 2)) as isize;
    let rgb = color.rgb_16();

    (addr as *mut u16).offset(index).write(rgb);
}

unsafe fn draw_pixel_24_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: Color) {
    let index = (x * 3 + y * pitch) as isize;
    let rgb = color.rgb_24();

    addr.offset(index).write((rgb & 0xff) as u8);
    addr.offset(index + 1).write(((rgb >> 8) & 0xff) as u8);
    addr.offset(index + 2).write(((rgb >> 16) & 0xff) as u8);
}

unsafe fn draw_pixel_32_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: Color) {
    let index = (x + y * (pitch / 4)) as isize;
    let rgb = color.rgb_32();

    (addr as *mut u32).offset(index).write(rgb);
}
