use input::keyboard::ReadKeyboardOption;
use stream::{DecodedInputStream, InputStream};

use crate::{keyboard, mouse};

pub fn sys_read_mouse() -> usize {
    match mouse() {
        Some(mouse) => mouse.read().unwrap_or(0x0) as usize,
        None => 0x0,
    }
}

/// TODO#9 docs
///
/// Author: Sebastian Keller
pub fn sys_read_keyboard(option: ReadKeyboardOption) -> isize {
    let keyboard = keyboard().expect("Failed to read from keyboard!");

    match option {
        ReadKeyboardOption::Raw => keyboard.read_byte() as isize,
        ReadKeyboardOption::Decode => keyboard.decoded_read_byte() as isize,
        ReadKeyboardOption::TryDecode => keyboard.decoded_try_read_byte().unwrap_or(0) as isize,
    }
}
