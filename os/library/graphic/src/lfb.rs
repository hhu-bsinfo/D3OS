use crate::color::Color;
use libm::Libm;
use unifont::get_glyph;

#[derive(Clone, Copy)]
pub struct LFB {
    buffer: *mut u8,
    pitch: u32,
    width: u32,
    height: u32,
    bpp: u8,

    pixel_drawer: PixelDrawer,

    is_dirty: bool,
}

unsafe impl Send for LFB {}
unsafe impl Sync for LFB {}

pub const DEFAULT_CHAR_WIDTH: u32 = 8;
pub const DEFAULT_CHAR_HEIGHT: u32 = 16;

macro_rules! keep_iterating_for_dim {
    ($stepsize:expr, $curr:expr, $other:expr) => {
        if $stepsize >= 0.0 {
            $curr <= $other as f32
        } else {
            $curr >= $other as f32
        }
    };
}

impl LFB {
    pub const fn new(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        let pixel_drawer: PixelDrawer = match bpp {
            15 => draw_pixel_15_bit,
            16 => draw_pixel_16_bit,
            24 => draw_pixel_24_bit,
            32 => draw_pixel_32_bit,
            _ => draw_pixel_stub,
        };

        Self {
            buffer,
            pitch,
            width,
            height,
            bpp,
            pixel_drawer,
            is_dirty: false,
        }
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

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn mark_not_dirty(&mut self) {
        self.is_dirty = false;
    }

    pub fn draw_line(&mut self, x1: u32, y1: u32, x2: u32, y2: u32, color: Color) {
        // Check if pixels are outside the framebuffer
        if x1 >= self.width || y1 >= self.height || x2 >= self.width || y2 >= self.height {
            return;
        }

        // Do not draw pixels with alpha = 0
        if color.alpha == 0 {
            return;
        }

        let (x_dist, y_dist) = (x2 as f32 - x1 as f32, y2 as f32 - y1 as f32);
        let hypot = Libm::<f32>::hypot(x_dist, y_dist);
        let (x_stepsize, y_stepsize) = (x_dist / hypot, y_dist / hypot);
        let (mut x_curr, mut y_curr) = (x1 as f32, y1 as f32);

        // Blend if necessary and draw pixel
        if color.alpha < 255 {
            while keep_iterating_for_dim!(x_stepsize, x_curr, x2)
                && keep_iterating_for_dim!(y_stepsize, y_curr, y2)
            {
                let (x_u32, y_u32) = (
                    Libm::<f32>::round(x_curr) as u32,
                    Libm::<f32>::round(y_curr) as u32,
                );
                unsafe {
                    (self.pixel_drawer)(
                        self.buffer,
                        self.pitch,
                        x_u32,
                        y_u32,
                        self.read_pixel(x_u32, y_u32).blend(color),
                    )
                };

                x_curr += x_stepsize;
                y_curr += y_stepsize;
            }
        } else {
            while keep_iterating_for_dim!(x_stepsize, x_curr, x2)
                && keep_iterating_for_dim!(y_stepsize, y_curr, y2)
            {
                let (x_u32, y_u32) = (
                    Libm::<f32>::round(x_curr) as u32,
                    Libm::<f32>::round(y_curr) as u32,
                );
                unsafe { (self.pixel_drawer)(self.buffer, self.pitch, x_u32, y_u32, color) };

                x_curr += x_stepsize;
                y_curr += y_stepsize;
            }
        }

        self.is_dirty = true;
    }

    #[inline]
    pub fn draw_pixel(&mut self, x: u32, y: u32, color: Color) {
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
            unsafe {
                (self.pixel_drawer)(
                    self.buffer,
                    self.pitch,
                    x,
                    y,
                    self.read_pixel(x, y).blend(color),
                )
            };
        } else {
            unsafe { (self.pixel_drawer)(self.buffer, self.pitch, x, y, color) };
        }

        self.is_dirty = true;
    }

    pub fn read_pixel(&self, x: u32, y: u32) -> Color {
        if x > self.width - 1 || y > self.height - 1 {
            panic!("LinearFrameBuffer: Trying to read a pixel out of bounds!");
        }

        let bpp = if self.bpp == 15 { 16 } else { self.bpp() };

        unsafe {
            let ptr = self
                .buffer
                .offset(((x * (bpp / 8) as u32) + y * self.pitch) as isize)
                as *const u32;
            Color::from_rgb(ptr.read(), self.bpp)
        }
    }

    pub fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        let end_x = x + width;
        let end_y = y + height;

        for i in y..end_y {
            for j in x..end_x {
                self.draw_pixel(j, i, color);
            }
        }
    }

    pub fn draw_char(&mut self, x: u32, y: u32, fg_color: Color, bg_color: Color, c: char) -> u32 {
        self.draw_char_scaled(x, y, 1, 1, fg_color, bg_color, c)
    }

    pub fn draw_char_scaled(
        &mut self,
        x: u32,
        y: u32,
        x_scale: u32,
        y_scale: u32,
        fg_color: Color,
        bg_color: Color,
        c: char,
    ) -> u32 {
        return match get_glyph(c) {
            Some(glyph) => {
                let mut x_offset = 0;
                let mut y_offset = 0;

                for row in 0..DEFAULT_CHAR_HEIGHT {
                    for col in 0..glyph.get_width() as u32 {
                        let color = match glyph.get_pixel(col as usize, row as usize) {
                            true => fg_color,
                            false => bg_color,
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
            }
            None => 0,
        };
    }

    pub fn draw_string(&mut self, x: u32, y: u32, fg_color: Color, bg_color: Color, string: &str) {
        self.draw_string_scaled(x, y, 1, 1, fg_color, bg_color, string);
    }

    pub fn draw_string_scaled(
        &mut self,
        x: u32,
        y: u32,
        x_scale: u32,
        y_scale: u32,
        fg_color: Color,
        bg_color: Color,
        string: &str,
    ) {
        for c in string.chars().enumerate() {
            self.draw_char_scaled(
                x + (c.0 as u32 * (8 * x_scale)),
                y,
                x_scale,
                y_scale,
                fg_color,
                bg_color,
                c.1,
            );
        }
    }

    pub fn clear(&self) {
        unsafe {
            self.buffer
                .write_bytes(0, (self.pitch * self.height) as usize);
        }
    }

    pub fn scroll_up(&mut self, lines: u32) {
        unsafe {
            // Move screen buffer upwards by the given amount of lines
            self.buffer.copy_from(
                self.buffer.offset((self.pitch * lines) as isize),
                (self.pitch * (self.height - lines)) as usize,
            );

            // Clear lower part of the screen
            self.buffer
                .offset((self.pitch * (self.height - lines)) as isize)
                .write_bytes(0, (self.pitch * lines) as usize);
        }

        self.is_dirty = true;
    }

    pub fn fill_triangle(
        &mut self,
        ((mut x1, mut y1), (mut x2, mut y2), (mut x3, mut y3)): (
            (u32, u32),
            (u32, u32),
            (u32, u32),
        ),
        color: Color,
    ) {
        // Sort vertices by y-coordinate ascending (y1 <= y2 <= y3)
        if y1 > y2 {
            core::mem::swap(&mut x1, &mut x2);
            core::mem::swap(&mut y1, &mut y2);
        }
        if y1 > y3 {
            core::mem::swap(&mut x1, &mut x3);
            core::mem::swap(&mut y1, &mut y3);
        }
        if y2 > y3 {
            core::mem::swap(&mut x2, &mut x3);
            core::mem::swap(&mut y2, &mut y3);
        }

        // Calculate slopes
        let dx1 = if y2 != y1 {
            (x2 as f32 - x1 as f32) / (y2 as f32 - y1 as f32)
        } else {
            0.0
        };
        let dx2 = if y3 != y1 {
            (x3 as f32 - x1 as f32) / (y3 as f32 - y1 as f32)
        } else {
            0.0
        };
        let dx3 = if y3 != y2 {
            (x3 as f32 - x2 as f32) / (y3 as f32 - y2 as f32)
        } else {
            0.0
        };

        // Draw lower part of the triangle
        let mut start_x = x1 as f32;
        let mut end_x = x1 as f32;
        for y in y1..=y2 {
            self.draw_horizontal_line(start_x, end_x, y, color);
            start_x += dx1;
            end_x += dx2;
        }

        // Draw upper part of the triangle
        start_x = x2 as f32;
        end_x = x1 as f32 + dx2 * (y2 as f32 - y1 as f32);
        for y in y2..=y3 {
            self.draw_horizontal_line(start_x, end_x, y, color);
            start_x += dx3;
            end_x += dx2;
        }
    }

    fn draw_horizontal_line(&mut self, start_x: f32, end_x: f32, y: u32, color: Color) {
        if y >= self.height {
            return; // y is out of bounds
        }

        let mut start_x = Libm::<f32>::round(start_x) as u32;
        let mut end_x = Libm::<f32>::round(end_x) as u32;

        if start_x > end_x {
            core::mem::swap(&mut start_x, &mut end_x);
        }

        start_x = core::cmp::max(0, core::cmp::min(self.width - 1, start_x));
        end_x = core::cmp::max(0, core::cmp::min(self.width - 1, end_x));

        for x in start_x..=end_x {
            if x < self.width && y < self.height {
                unsafe {
                    (self.pixel_drawer)(self.buffer, self.pitch, x, y, color);
                }
            }
        }

        self.is_dirty = true;
    }

    pub fn draw_bitmap(&mut self, x: u32, y: u32, data: &[Color], width: u32, height: u32) {
        for i in 0..height {
            for j in 0..width {
                let color = data[(i * width + j) as usize];
                self.draw_pixel(x + j as u32, y + i as u32, color);
            }
        }
    }

    pub fn draw_circle_bresenham(&mut self, center: (i32, i32), radius: i32, color: Color) {
        let (cx, cy) = center;
        let mut x = 0;
        let mut y = radius;
        let mut d = 3 - 2 * radius; // Entscheidungsvariable
    
        while x <= y {
            // Zeichne alle symmetrischen Punkte
            self.draw_pixel((cx + x) as u32, (cy + y) as u32, color);
            self.draw_pixel((cx - x) as u32, (cy + y) as u32, color);
            self.draw_pixel((cx + x) as u32, (cy - y) as u32, color);
            self.draw_pixel((cx - x) as u32, (cy - y) as u32, color);
            self.draw_pixel((cx + y) as u32, (cy + x) as u32, color);
            self.draw_pixel((cx - y) as u32, (cy + x) as u32, color);
            self.draw_pixel((cx + y) as u32, (cy - x) as u32, color);
            self.draw_pixel((cx - y) as u32, (cy - x) as u32, color);
    
            // Aktualisiere die Entscheidungsvariable und Positionen
            if d <= 0 {
                d += 4 * x + 6;
            } else {
                d += 4 * (x - y) + 10;
                y -= 1;
            }
            x += 1;
        }
    }

    pub fn draw_filled_circle_bresenham(
        &mut self,
        center: (i32, i32),
        radius: i32,
        color: Color,
    ) {
        let (cx, cy) = center;
        let mut x = 0;
        let mut y = radius;
        let mut d = 3 - 2 * radius; // Entscheidungsvariable
    
        while x <= y {
            // Zeichne horizontale Linien für die symmetrischen Teile des Kreises
            self.draw_horizontal_line(
                cx as f32 - x as f32,
                cx as f32 + x as f32,
                (cy + y) as u32,
                color,
            );
            self.draw_horizontal_line(
                cx as f32 - x as f32,
                cx as f32 + x as f32,
                (cy - y) as u32,
                color,
            );
            self.draw_horizontal_line(
                cx as f32 - y as f32,
                cx as f32 + y as f32,
                (cy + x) as u32,
                color,
            );
            self.draw_horizontal_line(
                cx as f32 - y as f32,
                cx as f32 + y as f32,
                (cy - x) as u32,
                color,
            );
    
            // Aktualisiere die Entscheidungsvariable und Positionen
            if d <= 0 {
                d += 4 * x + 6;
            } else {
                d += 4 * (x - y) + 10;
                y -= 1;
            }
            x += 1;
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

    unsafe { (addr as *mut u16).offset(index).write(rgb); }
}

unsafe fn draw_pixel_16_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: Color) {
    let index = (x + y * (pitch / 2)) as isize;
    let rgb = color.rgb_16();

    unsafe { (addr as *mut u16).offset(index).write(rgb); }
}

unsafe fn draw_pixel_24_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: Color) {
    let index = (x * 3 + y * pitch) as isize;
    let rgb = color.rgb_24();

    unsafe {
        addr.offset(index).write((rgb & 0xff) as u8);
        addr.offset(index + 1).write(((rgb >> 8) & 0xff) as u8);
        addr.offset(index + 2).write(((rgb >> 16) & 0xff) as u8);
    }
}

unsafe fn draw_pixel_32_bit(addr: *mut u8, pitch: u32, x: u32, y: u32, color: Color) {
    let index = (x + y * (pitch / 4)) as isize;
    let rgb = color.rgb_32();

    unsafe { (addr as *mut u32).offset(index).write(rgb); }
}
