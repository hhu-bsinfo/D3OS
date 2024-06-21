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

pub fn scale_rect_to_window(rel_rect_data: RectData, abs_rect_data: RectData) -> RectData {
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

    RectData {
        top_left: Vertex::new(
            (f64::from(rel_top_left.x) * ratios.0) as u32 + abs_rect_data.top_left.x,
            (f64::from(rel_top_left.y) * ratios.1) as u32 + abs_rect_data.top_left.y,
        ),
        width: (f64::from(rel_width) * ratios.0) as u32,
        height: (f64::from(rel_height) * ratios.1) as u32,
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
