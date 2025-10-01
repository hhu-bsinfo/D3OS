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
pub fn sys_read_keyboard(option: ReadKeyboardOption, blocking: bool) -> isize {
    let keyboard = keyboard().expect("Failed to read from keyboard!");

    match option {
        ReadKeyboardOption::Raw => (if blocking {
            keyboard.read_byte()
        } else {
            keyboard.read_byte_nb().unwrap_or_default()
        } as isize),
        ReadKeyboardOption::Decode => (if blocking {
            keyboard.decoded_read_byte()
        } else {
            keyboard.decoded_try_read_byte().unwrap_or_default()
        } as isize),
    }
}
