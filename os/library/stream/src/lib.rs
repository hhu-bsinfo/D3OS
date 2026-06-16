#![no_std]

use core::fmt;
use core::fmt::Write;
use core::ops::Deref;

use pc_keyboard::{KeyCode, KeyEvent, KeyState};

pub trait RawInputStream {
    /// Read a single event, blocking.
    fn read_event(&self) -> KeyEvent;
    /// Read a single event, non-blocking.
    fn read_event_nb(&self) -> Option<KeyEvent>;
    
}

/// Convert a key event to u16.
pub fn event_to_u16(event: KeyEvent) -> u16 {
    let mut res: u16 = (event.code as u8).into();
    res |= match event.state {
        KeyState::Up => 1,
        KeyState::Down => 2,
        KeyState::SingleShot => 0,
    } << 8;
    res
}

pub fn event_from_u16(raw: u16) -> KeyEvent {
    let code = unsafe { core::mem::transmute::<u8, KeyCode>(raw as u8) };
    let state = match raw >> 8 {
        0 => KeyState::SingleShot,
        1 => KeyState::Up,
        2 => KeyState::Down,
        state => panic!("invalid key state {state}"),
    };
    KeyEvent::new(code, state)
}

pub trait DecodedInputStream {
    fn decoded_read_byte(&self) -> i16;
    fn decoded_try_read_byte(&self) -> Option<i16>;
}

pub trait OutputStream: Send + Sync {
    fn write_byte(&self, b: u8);
    fn write_str(&self, string: &str);
}

// Implementation of the 'core::fmt::Write' trait for OutputStream
// Required to output formatted strings
// Requires only one function 'write_str'
impl Write for dyn OutputStream {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.deref().write_str(s);
        Ok(())
    }
}