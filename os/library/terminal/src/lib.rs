#![no_std]

pub mod read;
pub mod write;

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
    InputReaderWaiting = 1,
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
