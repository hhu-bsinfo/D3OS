use core::{
    fmt::Display,
    ops::{Add, AddAssign},
};

use crate::rect_data::RectData;

#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub x: u32,
    pub y: u32,
}

impl Add for Vertex {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x.saturating_add(rhs.x),
            y: self.y.saturating_add(rhs.y),
        }
    }
}

impl AddAssign for Vertex {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Vertex {
    pub fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    pub fn as_tuple(&self) -> (u32, u32) {
        (self.x, self.y)
    }

    pub fn add(&self, x_delta: u32, y_delta: u32) -> Self {
        Self {
            x: self.x.saturating_add(x_delta),
            y: self.y.saturating_add(y_delta),
        }
    }

    pub fn sub(&self, x_delta: u32, y_delta: u32) -> Self {
        Self {
            x: self.x.saturating_sub(x_delta),
            y: self.y.saturating_sub(y_delta),
        }
    }

    pub fn scale_by_rect_ratio(&self, old_rect_data: &RectData, new_rect_data: &RectData) -> Self {
        // Deltas between window-pos and vertex
        let delta_x = f64::from(self.x - old_rect_data.top_left.x);
        let delta_y = f64::from(self.y - old_rect_data.top_left.y);
        // Calculate scale factors
        let scale_x = f64::from(new_rect_data.width) / f64::from(old_rect_data.width);
        let scale_y = f64::from(new_rect_data.height) / f64::from(old_rect_data.height);

        Self {
            x: new_rect_data.top_left.x + ((delta_x * scale_x) as u32),
            y: new_rect_data.top_left.y + ((delta_y * scale_y) as u32),
        }
    }
}

impl Display for Vertex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "x: {}, y: {}", self.x, self.y)
    }
}
