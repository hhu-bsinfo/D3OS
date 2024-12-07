use alloc::vec::Vec;
use libm::floorf;

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