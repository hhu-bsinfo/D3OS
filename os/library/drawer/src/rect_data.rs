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
}
