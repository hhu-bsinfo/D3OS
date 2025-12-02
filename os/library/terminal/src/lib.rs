#![no_std]
extern crate alloc;

#[cfg(feature = "userspace")]
pub mod read;
#[cfg(feature = "userspace")]
pub mod write;

#[cfg(feature = "userspace")]
pub use pc_keyboard::{DecodedKey, KeyCode};

#[cfg(feature = "userspace")]
use logger::Logger;

use num_enum::{FromPrimitive, IntoPrimitive};
#[cfg(feature = "userspace")]
use spin::Once;

#[derive(Debug, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(usize)]
pub enum TerminalInputState {
    #[num_enum(default)]
    Idle = 0,
    Canonical = 1,
    Fluid = 2,
    Raw = 3,
}

#[derive(Debug, PartialEq, Clone, Copy, IntoPrimitive, FromPrimitive)]
#[repr(usize)]
pub enum TerminalMode {
    #[num_enum(default)]
    Canonical = 0,
    Fluid = 1,
    Raw = 2,
}

/*impl From<usize> for TerminalMode {
    fn from(b: usize) -> Self {
        match b {
            0 => TerminalMode::Canonical,
            1 => TerminalMode::Fluid,
            2 => TerminalMode::Raw,
            _ => panic!("Invalid value for TerminalMode: {}", b),
        }
    }
}*/

#[derive(Debug, PartialEq, Clone, Copy, IntoPrimitive, FromPrimitive)]
#[repr(u8)]
pub enum DecodedKeyType {
    #[num_enum(default)]
    Unicode = 0,
    RawKey = 1,
}

/*impl From<u8> for DecodedKeyType {
    fn from(b: u8) -> Self {
        match b {
            0 => DecodedKeyType::Unicode,
            1 => DecodedKeyType::RawKey,
            _ => panic!("Invalid value for DecodedKeyType: {}", b),
        }
    }
}*/

#[cfg(feature = "userspace")]
static LOGGER: Once<Logger> = Once::new();

#[cfg(feature = "userspace")]
pub fn init_logger() {
    use log::{set_logger, LevelFilter};

    LOGGER.call_once(Logger::new);
    set_logger(LOGGER.get().unwrap())
        .map(|()| log::set_max_level(LevelFilter::Debug))
        .expect("Failed to initialize logger!");
}
