use alloc::collections::{linked_list::CursorMut, LinkedList};
use drawer::{rect_data::RectData, vertex::Vertex};

use crate::SCREEN;

// TODO: Shouldn't this be inside a trait or something?
pub fn get_element_cursor_from_orderer<T: PartialEq>(
    linked_list: &mut LinkedList<T>,
    needle: T,
) -> Option<CursorMut<T>> {
    let mut cursor = linked_list.cursor_front_mut();
    while let Some(element) = cursor.current() {
        if *element == needle {
            return Some(cursor);
        }

        cursor.move_next();
    }

    return None;
}

/// Scales a rect (`rel_rect`) relative to a an absolute rect (`abs_container`) and returns the scaled absolute rect. \
/// The absolute rect will **always** be constrained by the absolute `min_dim` and `max_dim` sizes, disregarding the
/// aspect ratio if neccessary.
pub fn scale_rect_to_rect(
    rel_rect: RectData,
    abs_container: RectData,
    min_dim: (u32, u32),
    max_dim: (u32, u32),
    maintain_aspect_ratio: bool,
) -> RectData {
    let screen = SCREEN.get().unwrap();

    // Calculate container-to-screen ratios
    let ratios = (
        f64::from(abs_container.width) / f64::from(screen.0),
        f64::from(abs_container.height) / f64::from(screen.1),
    );

    let abs_size = {
        // Scale to absolute sizes
        let abs_width = if rel_rect.width == 0 {
            abs_container.width
        } else {
            (f64::from(rel_rect.width) * ratios.0) as u32
        };

        let abs_height = if rel_rect.height == 0 {
            abs_container.height
        } else {
            (f64::from(rel_rect.height) * ratios.1) as u32
        };

        if maintain_aspect_ratio {
            // Calculate the aspect ratio while taking stretching (w/h = 0) into account
            let aspect_ratio = match (rel_rect.width, rel_rect.height) {
                (0, 0) => f64::from(abs_container.width) / f64::from(abs_container.height),
                (0, h) => f64::from(abs_container.width) / f64::from(h),
                (w, 0) => f64::from(w) / f64::from(abs_container.height),
                (w, h) => f64::from(w) / f64::from(h),
            };
            
            let scaled_height = (f64::from(abs_width) / aspect_ratio) as u32;
            let scaled_width = (f64::from(abs_height) * aspect_ratio) as u32;

            // Shrink one side to maintain aspect ratio
            let scaled_size = if scaled_height <= abs_height {
                (abs_width, scaled_height)
            } else {
                (scaled_width, abs_height)
            };

            scaled_size
        } else {
            (abs_width, abs_height)
        }
    };

    RectData {
        top_left: Vertex::new(
            (f64::from(rel_rect.top_left.x) * ratios.0) as u32 + abs_container.top_left.x,
            (f64::from(rel_rect.top_left.y) * ratios.1) as u32 + abs_container.top_left.y,
        ),
        width: abs_size.0.clamp(min_dim.0, max_dim.0),
        height: abs_size.1.clamp(min_dim.1, max_dim.1),
    }
}

/// Scales a radius to be relative to an absolute rect.
pub fn scale_radius_to_rect(radius: u32, min_radius: u32, abs_rect: RectData) -> u32 {
    let screen = SCREEN.get().unwrap();

    let ratios = (
        f64::from(abs_rect.width) / f64::from(screen.0),
        f64::from(abs_rect.height) / f64::from(screen.1),
    );

    let scaled_radius = (f64::from(radius) * ratios.0.min(ratios.1)) as u32;

    scaled_radius.max(min_radius)
}

/// Scales a position relative to an absolute rect and returns the absolute position.
pub fn scale_pos_to_rect(rel_pos: Vertex, abs_rect: RectData) -> Vertex {
    let screen = SCREEN.get().unwrap();

    let ratios = (
        f64::from(abs_rect.width) / f64::from(screen.0),
        f64::from(abs_rect.height) / f64::from(screen.1),
    );

    Vertex::new(
        (f64::from(rel_pos.x) * ratios.0) as u32 + abs_rect.top_left.x,
        (f64::from(rel_pos.y) * ratios.1) as u32 + abs_rect.top_left.y,
    )
}

/// TODO: Is this even needed?
pub fn scale_font(
    _old_scale: &(u32, u32),
    _old_rect_data: &RectData,
    _new_rect_data: &RectData,
) -> (u32, u32) {
    return (1, 1);

    /*let ratios = (
        f64::from(new_rect_data.width) / f64::from(old_rect_data.width),
        f64::from(new_rect_data.height) / f64::from(old_rect_data.height),
    );

    (
        ((f64::from(old_scale.0) * ratios.0) as u32).max(1),
        ((f64::from(old_scale.1) * ratios.1) as u32).max(1),
    )*/
}
