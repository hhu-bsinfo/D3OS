use crate::mouse;

pub fn sys_read_mouse() -> usize {
    match mouse() {
        Some(mouse) => mouse.read().unwrap_or(0x0) as usize,
        None => 0x0,
    }
}
