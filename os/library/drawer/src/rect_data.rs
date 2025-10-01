use core::fmt::Display;

use crate::vertex::Vertex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectData {
    pub top_left: Vertex,
    pub width: u32,
    pub height: u32,
}

impl Display for RectData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "x: {}, y: {}, width: {}, height: {}",
            self.top_left.x, self.top_left.y, self.width, self.height
        )
    }
}

impl RectData {
    pub fn zero() -> Self {
        RectData { top_left: Vertex::zero(), width: 0, height: 0 }
    }

    pub fn sub_border(&self) -> Self {
        let mut new_rect = self.clone();
        new_rect.top_left += Vertex::new(1, 1);
        new_rect.width -= 2;
        new_rect.height -= 2;

        return new_rect;
    }

    /// Scale this RectData to fit into the new window size
    pub fn scale_dimensions(
        &self,
        old_window: &RectData,
        new_window: &RectData,
        min_dim: Option<(u32, u32)>,
    ) -> RectData {
        // Calculate scale factors
        let scale_x = f64::from(new_window.width) / f64::from(old_window.width);
        let scale_y = f64::from(new_window.height) / f64::from(old_window.height);
        let min_dim = min_dim.unwrap_or((0, 0));

        return RectData {
            top_left: self.top_left,
            width: ((f64::from(self.width) * scale_x) as u32).max(min_dim.0),
            height: ((f64::from(self.height) * scale_y) as u32).max(min_dim.1),
        };
    }

    pub fn intersects(&self, other: &RectData) -> bool {
        let self_top_left = self.top_left;
        let self_bottom_right = self.top_left.add(self.width, self.height);

        let other_top_left = other.top_left;
        let other_bottom_right = other.top_left.add(other.width, other.height);

        let x_overlap = self_bottom_right.x >= other_top_left.x && other_bottom_right.x >= self_top_left.x;
        let y_overlap = self_bottom_right.y >= other_top_left.y && other_bottom_right.y >= self_top_left.y;
        
        x_overlap && y_overlap
    }

    pub fn contains_vertex(&self, vertex: &Vertex) -> bool {
        let self_top_left = self.top_left;
        let self_bottom_right = self.top_left.add(self.width, self.height);

        vertex.x >= self_top_left.x && vertex.x <= self_bottom_right.x &&
        vertex.y >= self_top_left.y && vertex.y <= self_bottom_right.y
    }
}
