use crate::mouse;

pub fn sys_read_mouse() -> usize {
    match mouse().expect("Failed to read from mouse!").read() {
        Some(v) => v as usize,
        None => 0x0,
    }
}