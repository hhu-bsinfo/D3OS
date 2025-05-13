use graphic::{bitmap::Bitmap, color::Color, lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH}};
use alloc::string::String;
use text_buffer::TextBuffer;
//Julius Drodofsky
pub enum View{
    Simple {
        font_scale: u32,
        fg_color: Color,
        bg_color: Color,
    }
}

impl View {
    fn render_simple(&self, text: &TextBuffer, buffer: &mut Bitmap, font_scale: u32, fg_color: Color, bg_color: Color) {
        let mut x = 0;
        let mut y = 0;
        let mut i: usize = 0;
         while let Some(c) = text.get_char(i) {
            if(c=='\n'){
                y+= DEFAULT_CHAR_HEIGHT;
                x=0;
                continue;
            }
            if (buffer.width-x < DEFAULT_CHAR_WIDTH) {
                x=0;
                y+= DEFAULT_CHAR_HEIGHT;
            }
            x += buffer.draw_char_scaled(x, y, font_scale , font_scale, fg_color,bg_color, c);
            i+=1;
        }

    }
    pub fn render(&self, text: &TextBuffer, buffer: &mut Bitmap) {
        match self {
            View::Simple { font_scale, fg_color, bg_color }  => self.render_simple(text, buffer, *font_scale, *fg_color, *bg_color),
        }
        
    }
}