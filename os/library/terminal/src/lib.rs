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
