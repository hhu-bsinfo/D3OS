use pc_keyboard::KeyEvent;
use stream::event_from_u16;
use syscall::{SystemCall, syscall};

pub enum ReadKeyboardOption {
    Raw,
    Decode,
}

/// Read raw byte from keyboard.
pub fn read_raw(blocking: bool) -> Option<KeyEvent> {
    let option = ReadKeyboardOption::Raw as usize;
    let result = syscall(SystemCall::KeyboardRead, &[option, blocking as usize]);

    match result {
        Ok(0) => None,
        Ok(raw) => Some(event_from_u16(raw.try_into().unwrap())),
        Err(_) => None,
    }
}
