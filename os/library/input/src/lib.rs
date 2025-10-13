#![no_std]

pub enum ReadKeyboardOption {
    Raw,
    Decode,
}

#[cfg(feature = "userspace")]
pub mod keyboard;
#[cfg(feature = "userspace")]
pub mod mouse;
