use graphic::{bitmap::{self, Bitmap}, color::{Color, YELLOW}, lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH}};
use alloc::string::String;
use text_buffer::TextBuffer;

use super::model::Document;
//Julius Drodofsky
pub enum View{
    Simple {
        font_scale: u32,
        fg_color: Color,
        bg_color: Color,
    }
}

impl View {
    fn render_simple(&self, document: &Document, buffer: &mut Bitmap, font_scale: u32, fg_color: Color, bg_color: Color) {
        let mut x = 0;
        let mut y = 0;
        let mut i: usize = 0;
        buffer.clear(bg_color);
         while let Some(c) = document.text_buffer().get_char(i) {
            if i==document.caret(){
                buffer.draw_line(x, y, x, y+DEFAULT_CHAR_HEIGHT, YELLOW);
            }
            if c=='\n'{
                y+= DEFAULT_CHAR_HEIGHT;
                x=0;
                i+=1;
                continue;
            }
            if buffer.width-x+1 < DEFAULT_CHAR_WIDTH {
                x=0;
                y+= DEFAULT_CHAR_HEIGHT;
            }
            x += buffer.draw_char_scaled(x+1, y, font_scale , font_scale, fg_color,bg_color, c)+1;
            i+=1;
        }
            if i==document.caret(){
                buffer.draw_line(x, y, x, y+DEFAULT_CHAR_HEIGHT, YELLOW);
            }

    }
    pub fn render(&self, document: &Document, buffer: &mut Bitmap) {
        match self {
            View::Simple { font_scale, fg_color, bg_color }  => self.render_simple(document, buffer, *font_scale, *fg_color, *bg_color),
        }
        
    }
}