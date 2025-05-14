use core::fmt;
use core::fmt::Write;
use core::ops::Deref;
use stream::{InputStream, OutputStream};

pub trait Terminal: OutputStream + InputStream {
    fn clear(&self);
}

// Implementation of the 'core::fmt::Write' trait for our Terminal
// Required to output formatted strings
// Requires only one function 'write_str'
impl Write for dyn Terminal {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.deref().write_str(s);
        Ok(())
    }
}
