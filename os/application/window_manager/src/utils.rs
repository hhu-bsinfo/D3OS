use alloc::collections::{linked_list::CursorMut, LinkedList};
use drawer::{rect_data::RectData, vertex::Vertex};

use crate::SCREEN;

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

pub fn scale_rect_to_window(
    rel_rect_data: RectData,
    abs_rect_data: RectData,
    min_dim: (u32, u32),
    max_dim: (u32, u32),
    aspect_ratio: f64,
) -> RectData {
    let RectData {
        top_left: rel_top_left,
        width: rel_width,
        height: rel_height,
    } = rel_rect_data;

    let screen = SCREEN.get().unwrap();

    let ratios = (
        f64::from(abs_rect_data.width) / f64::from(screen.0),
        f64::from(abs_rect_data.height) / f64::from(screen.1),
    );

    let mut scaled_width = ((f64::from(rel_width) * ratios.0) as u32).max(min_dim.0);
    let mut scaled_height = ((f64::from(rel_height) * ratios.1) as u32).max(min_dim.1);

    // Erzwinge das Aspect Ratio
    let calculated_height = (f64::from(scaled_width) / aspect_ratio) as u32;
    let calculated_width = (f64::from(scaled_height) * aspect_ratio) as u32;

    if calculated_height <= scaled_height {
        scaled_height = calculated_height;
    } else {
        scaled_width = calculated_width;
    }

    // Begrenze auf maximale Dimensionen
    if scaled_width > max_dim.0 {
        scaled_width = max_dim.0;
        scaled_height = (f64::from(scaled_width) / aspect_ratio) as u32;
    }
    if scaled_height > max_dim.1 {
        scaled_height = max_dim.1;
        scaled_width = (f64::from(scaled_height) * aspect_ratio) as u32;
    }

    RectData {
        top_left: Vertex::new(
            (f64::from(rel_top_left.x) * ratios.0) as u32 + abs_rect_data.top_left.x,
            (f64::from(rel_top_left.y) * ratios.1) as u32 + abs_rect_data.top_left.y,
        ),
        width: scaled_width.max(min_dim.0),
        height: scaled_height.max(min_dim.1),
    }
}

pub fn scale_pos_to_window(rel_pos: Vertex, abs_window_rect_data: RectData) -> Vertex {
    let screen = SCREEN.get().unwrap();

    let ratios = (
        f64::from(abs_window_rect_data.width) / f64::from(screen.0),
        f64::from(abs_window_rect_data.height) / f64::from(screen.1),
    );

    Vertex::new(
        (f64::from(rel_pos.x) * ratios.0) as u32 + abs_window_rect_data.top_left.x,
        (f64::from(rel_pos.y) * ratios.1) as u32 + abs_window_rect_data.top_left.y,
    )
}

#[allow(unused_variables, unreachable_code)]
pub fn scale_font(
    old_scale: &(u32, u32),
    old_rect_data: &RectData,
    new_rect_data: &RectData,
) -> (u32, u32) {
    return (1, 1);
    let ratios = (
        f64::from(new_rect_data.width) / f64::from(old_rect_data.width),
        f64::from(new_rect_data.height) / f64::from(old_rect_data.height),
    );

    (
        ((f64::from(old_scale.0) * ratios.0) as u32).max(1),
        ((f64::from(old_scale.1) * ratios.1) as u32).max(1),
    )
}
