use alloc::collections::{linked_list::CursorMut, LinkedList};

pub fn get_element_cursor_from_orderer<T: PartialEq>(
    linked_list: &mut LinkedList<T>,
    needle: T,
) -> Option<CursorMut<T>> {
    let mut current = linked_list.cursor_front_mut();
    while let Some(element) = current.current() {
        if *element == needle {
            return Some(current);
        }

        current.move_next();
    }

    return None;
}
