#![no_std]

extern crate alloc;
extern crate pc_keyboard;

pub mod read;
pub mod write;

pub use pc_keyboard::DecodedKey;
pub use pc_keyboard::KeyCode;

use num_enum::{FromPrimitive, IntoPrimitive};

#[derive(Debug, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(usize)]
pub enum TerminalInputState {
    #[num_enum(default)]
    Idle = 0,
    Canonical = 1,
    Fluid = 2,
    Raw = 3,
}

#[derive(Debug, PartialEq, IntoPrimitive, FromPrimitive, Clone, Copy)]
#[repr(usize)]
pub enum TerminalMode {
    #[num_enum(default)]
    Canonical = 0,
    Fluid = 1,
    Raw = 2,
}

#[derive(Debug, PartialEq, IntoPrimitive, FromPrimitive, Clone, Copy)]
#[repr(u8)]
pub enum DecodedKeyType {
    #[num_enum(default)]
    Unicode = 0,
    RawKey = 1,
}
