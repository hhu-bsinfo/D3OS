use graphic::{bitmap::Bitmap, color::Color};
use alloc::string::String;
//Julius Drodofsky
pub enum View{
    Simple {
        font_scale: u32,
        fg_color: Color,
        bg_color: Color,
    }
}

impl View {
    fn render_simple(&self, text: &String, buffer: &mut Bitmap, font_scale: u32, fg_color: Color, bg_color: Color) {
        let mut x = 0;
        let mut y = 0;
        for c in  text.chars(){
            x += buffer.draw_char_scaled(x, 0, font_scale , font_scale, fg_color,bg_color, c);
        }

    }
    pub fn render(&self, text: &String, buffer: &mut Bitmap) {
        match self {
            View::Simple { font_scale, fg_color, bg_color }  => self.render_simple(text, buffer, *font_scale, *fg_color, *bg_color),
        }
        
    }
}