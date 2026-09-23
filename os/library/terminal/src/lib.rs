#![no_std]
extern crate alloc;

#[cfg(feature = "userspace")]
pub mod read;
#[cfg(feature = "userspace")]
pub mod write;

#[cfg(feature = "userspace")]
pub use pc_keyboard::{DecodedKey, KeyCode};

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
