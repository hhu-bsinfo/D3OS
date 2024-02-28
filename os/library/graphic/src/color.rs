use core::marker::Copy;

/**
 * Convert RGB-colors into their 1-, 2-, 4-, 8-, 15-, 16-, 24-, and 32-Bit representations.
 * Provides the possibility to blend to transparent colors.
 *
 * 32-Bit:
 *  Alpha     Red     Green     Blue
 * XXXXXXXX XXXXXXXX XXXXXXXX XXXXXXXX
 *
 * 24-Bit:
 *   Red     Green     Blue
 * XXXXXXXX XXXXXXXX XXXXXXXX
 *
 * 16-Bit:
 *  Red  Green  Blue
 * XXXXX XXXXXX XXXXX
 *
 * 15-Bit:
 *  Red  Green Blue
 * XXXXX XXXXX XXXXX
 */
#[derive(Copy, Clone)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

const BRIGHTNESS_SHIFT: u8 = 85;

pub const INVISIBLE: Color = Color { red: 0, green: 0, blue: 0, alpha: 0 };

// ANSI colors
pub const BLACK: Color = Color { red: 0, green: 0, blue: 0, alpha: 255 };
pub const RED: Color = Color { red: 170, green: 0, blue: 0, alpha: 255 };
pub const GREEN: Color = Color { red: 0, green: 170, blue: 0, alpha: 255 };
pub const YELLOW: Color = Color { red: 170, green: 170, blue: 0, alpha: 255 };
pub const BROWN: Color = Color { red: 170, green: 85, blue: 0, alpha: 255 };
pub const BLUE: Color = Color { red: 0, green: 0, blue: 170, alpha: 255 };
pub const MAGENTA: Color = Color { red: 170, green: 0, blue: 170, alpha: 255 };
pub const CYAN: Color = Color { red: 0, green: 170, blue: 170, alpha: 255 };
pub const WHITE: Color = Color { red: 170, green: 170, blue: 170, alpha: 255, };

// Arbitrary colors
pub const HHU_BLUE: Color = Color { red: 0, green: 106, blue: 179, alpha: 255 };
pub const HHU_GREEN: Color = Color { red: 140, green: 177, blue: 16, alpha: 255 };

impl Color {
    pub const fn from_rgb(rgb: u32, bpp: u8) -> Color {
        match bpp {
            32 => return Color::from_rgb_32(rgb),
            24 => return Color::from_rgb_24(rgb),
            16 => return Color::from_rgb_16(rgb as u16),
            15 => return Color::from_rgb_15(rgb as u16),
            _ => panic!("Color: Invalid bpp!"),
        }
    }

    pub const fn from_rgb_32(rgba: u32) -> Color {
        let alpha: u8 = ((rgba & 0xff000000) >> 24) as u8;
        let red: u8 = ((rgba & 0x00ff0000) >> 16) as u8;
        let green: u8 = ((rgba & 0x0000ff00) >> 8) as u8;
        let blue: u8 = (rgba & 0x000000ff) as u8;

        Self { red, green, blue, alpha }
    }

    pub const fn from_rgb_24(rgb: u32) -> Color {
        let red: u8 = ((rgb & 0x00ff0000) >> 16) as u8;
        let green: u8 = ((rgb & 0x0000ff00) >> 8) as u8;
        let blue: u8 = (rgb & 0x000000ff) as u8;

        Self { red, green, blue, alpha: 0 }
    }

    pub const fn from_rgb_16(rgb: u16) -> Color {
        let red: u8 = (((rgb & 0xf800) >> 11) * (256 / 32)) as u8;
        let green: u8 = (((rgb & 0x07e0) >> 5) * (256 / 64)) as u8;
        let blue: u8 = ((rgb & 0x001f) * (256 / 32)) as u8;

        Self { red, green, blue, alpha: 0 }
    }

    pub const fn from_rgb_15(rgb: u16) -> Color {
        let red: u8 = (((rgb & 0x7c00) >> 10) * (256 / 32)) as u8;
        let green: u8 = (((rgb & 0x03e0) >> 5) * (256 / 32)) as u8;
        let blue: u8 = ((rgb & 0x001f) * (256 / 32)) as u8;

        Self { red, green, blue, alpha: 0 }
    }

    pub const fn rgb_32(&self) -> u32 {
        ((self.alpha as u32) << 24) | ((self.red as u32) << 16) | ((self.green as u32) << 8) | ((self.blue) as u32)
    }

    pub const fn rgb_24(&self) -> u32 {
        ((self.red as u32) << 16) | ((self.green as u32) << 8) | ((self.blue) as u32)
    }

    pub const fn rgb_16(&self) -> u16 {
        ((self.blue as u16) >> 3) | (((self.green as u16) >> 2) << 5) | (((self.red as u16) >> 3) << 11)
    }

    pub const fn rgb_15(&self) -> u16 {
        ((self.blue as u16) >> 3) | (((self.green as u16) >> 3) << 5) | (((self.red as u16) >> 3) << 10)
    }

    pub const fn bright(&self) -> Color {
        let mut r: u16 = self.red as u16 + BRIGHTNESS_SHIFT as u16;
        let mut g: u16 = self.green as u16 + BRIGHTNESS_SHIFT as u16;
        let mut b: u16 = self.blue as u16 + BRIGHTNESS_SHIFT as u16;

        if r > 0xff {
            r = 0xff;
        }

        if g > 0xff {
            g = 0xff;
        }

        if b > 0xff {
            b = 0xff;
        }

        Self { red: r as u8, green: g as u8, blue: b as u8, alpha: self.alpha, }
    }

    pub const fn dim(&self) -> Color {
        let mut r: i16 = self.red as i16 - BRIGHTNESS_SHIFT as i16;
        let mut g: i16 = self.green as i16 - BRIGHTNESS_SHIFT as i16;
        let mut b: i16 = self.blue as i16 - BRIGHTNESS_SHIFT as i16;

        if r < 0 {
            r = 0;
        }

        if g < 0 {
            g = 0;
        }

        if b < 0 {
            b = 0;
        }

        Self { red: r as u8, green: g as u8, blue: b as u8, alpha: self.alpha, }
    }

    pub const fn with_alpha(&self, alpha: u8) -> Self {
        Self { red: self.red, green: self.green, blue: self.blue, alpha, }
    }

    pub fn blend(&self, color: Color) -> Color {
        if color.alpha == 0 {
            return Self { red: self.red, green: self.green, blue: self.blue, alpha: self.alpha, };
        }

        if color.alpha == 0xff {
            return Self { red: color.red, green: color.green, blue: color.blue, alpha: color.alpha, };
        }

        if self.alpha == 0 {
            return BLACK.blend(color);
        }

        let alpha1: f64 = (color.alpha as f64) / 255.0;
        let alpha2: f64 = (self.alpha as f64) / 255.0;
        let alpha3: f64 = alpha1 + (1.0 - alpha1) * alpha2;

        let r: u8 = ((1.0 / alpha3) * (alpha1 * color.red as f64 + (1.0 - alpha1) * alpha2 * self.red as f64)) as u8;
        let g: u8 = ((1.0 / alpha3) * (alpha1 * color.green as f64 + (1.0 - alpha1) * alpha2 * self.green as f64)) as u8;
        let b: u8 = ((1.0 / alpha3) * (alpha1 * color.blue as f64 + (1.0 - alpha1) * alpha2 * self.blue as f64)) as u8;
        let a: u8 = (alpha3 * 255.0) as u8;

        Self { red: r, green: g, blue: b, alpha: a }
    }
}
