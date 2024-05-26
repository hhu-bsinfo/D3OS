use drawer::drawer::{Drawer, Vertex};
use graphic::color;

#[derive(Debug)]
pub(crate) struct Window {
    pub(crate) id: usize,
    pub(crate) partner_id: Option<usize>,
    pub(crate) pos: Vertex,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Window {
    pub(crate) fn new(id: usize, partner_id: Option<usize>, pos: Vertex, width: u32, height: u32) -> Self {
        Self {
            id,
            partner_id,
            pos,
            width,
            height,
        }
    }

    pub(crate) fn draw(&self, focused_window_id: Option<usize>) {
        let color = if focused_window_id.is_some_and(|focused| focused == self.id) {
            color::YELLOW
        } else {
            color::WHITE
        };
        Drawer::draw_rectangle(
            Vertex::new(self.pos.x, self.pos.y),
            Vertex::new(self.pos.x + self.width, self.pos.y + self.height),
            color,
        );
    }
}