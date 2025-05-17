use graphic::color::{self, Color};

pub struct ColorState {
    pub fg_color: Color,
    pub bg_color: Color,
    pub fg_base_color: Color,
    pub bg_base_color: Color,
    pub fg_bright: bool,
    pub bg_bright: bool,
    pub invert: bool,
    pub bright: bool,
    pub dim: bool,
}

impl ColorState {
    pub const fn new() -> Self {
        Self {
            fg_color: color::WHITE,
            bg_color: color::BLACK,
            fg_base_color: color::WHITE,
            bg_base_color: color::BLACK,
            fg_bright: false,
            bg_bright: false,
            invert: false,
            bright: false,
            dim: false,
        }
    }
}
