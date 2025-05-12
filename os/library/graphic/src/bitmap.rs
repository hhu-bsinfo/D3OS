use alloc::vec::Vec;
use libm::floorf;
use unifont::get_glyph;
use crate::lfb::DEFAULT_CHAR_HEIGHT;

use crate::color::Color;

#[derive(Clone, Copy)]
pub enum ScalingMode {
    NearestNeighbor,
    Bilinear,
}

#[derive(Clone)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<Color>,
}

impl Bitmap {
    pub fn scale(&self, target_width: u32, target_height: u32, mode: ScalingMode) -> Bitmap {
        match mode {
            ScalingMode::NearestNeighbor => self.scale_nearest_neighbor(target_width, target_height),
            ScalingMode::Bilinear => self.scale_bilinear(target_width, target_height),
        }
    }

    // Julius Drodofsky
    pub fn read_pixel(&self, x: u32, y: u32) -> Color {
        match self.data.get((y*self.width+x) as usize){
            Some(c) => *c,
            None => panic!("Bitmap: Trying to read a pixel out of bounds!")
        }
    }

    #[inline]  //Julius Drodofsky
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
            let blend = self.read_pixel(x, y).blend(color);
            self.data[(self.width*y+x) as usize] = blend;           
        } else {
            self.data[(self.width*y+x) as usize] = color;           
        }
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

    // Julius Drodofsky
    pub fn clear(&mut self, color: Color) {
        for i in 0..self.width*self.height {
            self.data[i as usize] = color;
        }
    }

        // schnell aber schlechte Qualität
    fn scale_nearest_neighbor(&self, target_width: u32, target_height: u32) -> Bitmap {
        if target_height == self.height && target_width == self.width {
            return self.clone();
        }

        let mut scaled_data = Vec::with_capacity((target_width * target_height) as usize);

        // neirest neighbor scaling
        for y in 0..target_height {
            for x in 0..target_width {
                let orig_x = x * self.width / target_width;
                let orig_y = y * self.height / target_height;
                scaled_data.push(self.data[(orig_y * self.width + orig_x) as usize]);
            }
        }

        Bitmap {
            width: target_width,
            height: target_height,
            data: scaled_data,
        }
    }

    // langsam aber gute Qualität
    fn scale_bilinear(&self, target_width: u32, target_height: u32) -> Bitmap {
        if target_height == self.height && target_width == self.width {
            return self.clone();
        }

        let mut scaled_data = Vec::with_capacity((target_width * target_height) as usize);

        // bilinear scaling
        for y in 0..target_height {
            let fy = (y as f32) * (self.height as f32) / (target_height as f32);
            let y1 = floorf(fy) as u32;
            let y2 = (y1 + 1).min(self.height - 1);
            let ty = fy - (y1 as f32);

            for x in 0..target_width {
                let fx = (x as f32) * (self.width as f32) / (target_width as f32);
                let x1 = floorf(fx) as u32;
                let x2 = (x1 + 1).min(self.width - 1);
                let tx = fx - (x1 as f32);

                // farbinterpolation
                let c1 = self.data[(y1 * self.width + x1) as usize];
                let c2 = self.data[(y1 * self.width + x2) as usize];
                let c3 = self.data[(y2 * self.width + x1) as usize];
                let c4 = self.data[(y2 * self.width + x2) as usize];

                let red = ((c1.red as f32) * (1.0 - tx) * (1.0 - ty)
                    + (c2.red as f32) * tx * (1.0 - ty)
                    + (c3.red as f32) * (1.0 - tx) * ty
                    + (c4.red as f32) * tx * ty) as u8;

                let green = ((c1.green as f32) * (1.0 - tx) * (1.0 - ty)
                    + (c2.green as f32) * tx * (1.0 - ty)
                    + (c3.green as f32) * (1.0 - tx) * ty
                    + (c4.green as f32) * tx * ty) as u8;

                let blue = ((c1.blue as f32) * (1.0 - tx) * (1.0 - ty)
                    + (c2.blue as f32) * tx * (1.0 - ty)
                    + (c3.blue as f32) * (1.0 - tx) * ty
                    + (c4.blue as f32) * tx * ty) as u8;
                
                let alpha = ((c1.alpha as f32) * (1.0 - tx) * (1.0 - ty)
                    + (c2.alpha as f32) * tx * (1.0 - ty)
                    + (c3.alpha as f32) * (1.0 - tx) * ty
                    + (c4.alpha as f32) * tx * ty) as u8;

                scaled_data.push(Color {red, green, blue, alpha});
            }
        }

        Bitmap {
            width: target_width,
            height: target_height,
            data: scaled_data,
        }
    }
}