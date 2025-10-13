use input::ReadKeyboardOption;
use stream::{event_to_u16, DecodedInputStream, RawInputStream};

use crate::{keyboard, mouse};

pub fn sys_read_mouse() -> usize {
    match mouse() {
        Some(mouse) => mouse.read().unwrap_or(0x0) as usize,
        None => 0x0,
    }
}

/// SystemCall implementation for SystemCall::KeyboardRead.
/// Reads from keyboard with given mode (Raw or Decoded).
pub fn sys_read_keyboard(option: ReadKeyboardOption, blocking: bool) -> isize {
    let keyboard = keyboard().expect("Failed to read from keyboard!");

    match option {
        ReadKeyboardOption::Raw => {
            let event = if blocking {
                Some(keyboard.read_event())
            } else {
                keyboard.read_event_nb()
            };
            if let Some(event) = event {
                event_to_u16(event).try_into().unwrap()
            } else { 0 }
        },
        ReadKeyboardOption::Decode => (if blocking {
            keyboard.decoded_read_byte()
        } else {
            keyboard.decoded_try_read_byte().unwrap_or_default()
        } as isize),
    }
}
