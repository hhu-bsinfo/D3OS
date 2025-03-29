use crate::mouse;

pub fn sys_read_mouse() -> isize {
    match mouse().expect("Failed to read from mouse!").read() {
        Some(v) => v as isize,
        None => 0x0,
    }
}