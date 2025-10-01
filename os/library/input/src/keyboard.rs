use syscall::{SystemCall, syscall};

pub enum ReadKeyboardOption {
    Raw,
    Decode,
}

/// Read raw byte from keyboard.
///
/// Author: Sebastian Keller
pub fn read_raw(blocking: bool) -> Option<u8> {
    let option = ReadKeyboardOption::Raw as usize;
    let result = syscall(SystemCall::KeyboardRead, &[option, blocking as usize]);

    match result {
        Ok(0) => None,
        Ok(code) => Some(u8::try_from(code).expect("Code must be valid u8")),
        Err(_) => None,
    }
}
