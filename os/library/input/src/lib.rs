#![no_std]

#[repr(C)]
pub enum ReadKeyboardOption {
    Raw,
    Decode,
}

#[cfg(feature = "userspace")]
pub mod keyboard;
#[cfg(feature = "userspace")]
pub mod mouse;
