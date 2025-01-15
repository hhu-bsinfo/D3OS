#![no_std]

use core::fmt;
use core::fmt::Write;
use core::ops::Deref;

pub trait InputStream {
    fn read_byte(&self) -> i16;
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

pub fn strlen(str: *const u8) -> usize {
    let mut len = 0;
    unsafe {
        while *str.add(len) != 0 {
            len += 1;
        }
    }

    len
}
