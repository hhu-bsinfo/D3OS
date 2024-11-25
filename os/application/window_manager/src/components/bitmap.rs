use alloc::{boxed::Box, rc::Rc, vec::Vec};
use drawer::{drawer::Drawer, rect_data::RectData, vertex::Vertex};
use graphic::{bitmap::Bitmap, color::Color};
use spin::RwLock;

use super::component::Component;

pub struct BitmapGraphic {
    pos: Vertex,
    bitmap: Bitmap,
}

impl BitmapGraphic {
    pub fn new(pos: Vertex, bitmap: Bitmap) -> Self {
        Self { pos, bitmap }
    }
}

impl Component for BitmapGraphic {
    fn draw(&self, fg_color: Color, bg_color: Option<Color>) {
        Drawer::draw_bitmap(self.pos, &self.bitmap);
    }

    fn consume_keyboard_press(&mut self, keyboard_press: char) -> bool {
        return false;
    }

    fn rescale_after_move(&mut self, new_rect_data: drawer::rect_data::RectData) {
        //
    }

    fn rescale_after_split(&mut self, old_rect_data: drawer::rect_data::RectData, new_rect_data: drawer::rect_data::RectData) {
        //
    }

    fn get_abs_rect_data(&self) -> RectData {
        RectData {
            top_left: self.pos,
            width: self.bitmap.width,
            height: self.bitmap.height,
        }
    }

    fn get_redraw_components(&self) -> Vec<Rc<RwLock<Box<dyn Component>>>> {
        Vec::new()
    }
}