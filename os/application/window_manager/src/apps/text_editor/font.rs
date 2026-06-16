use graphic::color::Color;
#[derive(Debug, Clone, Copy)]
pub struct Font {
    pub scale: u32,
    pub fg_color: Color,
    pub bg_color: Color,
    pub char_width: u32,
    pub char_height: u32,
}

impl Font {
    pub fn add_scale(&self, add: u32) -> Font {
        let mut ret = *self;
        ret.scale += add;
        ret
    }
}
