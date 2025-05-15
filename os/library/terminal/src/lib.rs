#![no_std]

extern crate alloc;
extern crate pc_keyboard;

pub mod read;
pub mod write;

pub use pc_keyboard::DecodedKey;
pub use pc_keyboard::KeyCode;

use num_enum::{FromPrimitive, IntoPrimitive};

pub enum Application {
    Shell,
    WindowManager,
}

#[derive(Debug, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(usize)]
pub enum TerminalInputState {
    #[num_enum(default)]
    Idle = 0,
    InputReaderAwaitsCooked = 1,
    InputReaderAwaitsMixed = 2,
    InputReaderAwaitsRaw = 3,
}

#[derive(Debug, PartialEq, IntoPrimitive, FromPrimitive, Clone, Copy)]
#[repr(usize)]
pub enum TerminalMode {
    #[num_enum(default)]
    Cooked = 0,
    Mixed = 1,
    Raw = 2,
}

#[derive(Debug, PartialEq, IntoPrimitive, FromPrimitive, Clone, Copy)]
#[repr(u8)]
pub enum DecodedKeyType {
    #[num_enum(default)]
    Unicode = 0,
    RawKey = 1,
}
