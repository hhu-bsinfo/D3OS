use input::keyboard::ReadKeyboardOption;
use stream::{DecodedInputStream, InputStream};

use crate::{keyboard, mouse};

pub fn sys_read_mouse() -> usize {
    match mouse() {
        Some(mouse) => mouse.read().unwrap_or(0x0) as usize,
        None => 0x0,
    }
}

/// SystemCall implementation for SystemCall::KeyboardRead.
/// Reads from keyboard with given mode (Raw or Decoded).
///
/// Author: Sebastian Keller
pub fn sys_read_keyboard(option: ReadKeyboardOption) -> isize {
    let keyboard = keyboard().expect("Failed to read from keyboard!");

    match option {
        ReadKeyboardOption::Raw => keyboard.read_byte() as isize,
        ReadKeyboardOption::Decode => keyboard.decoded_read_byte() as isize,
    }
}
