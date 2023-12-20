use core::fmt;
use core::fmt::Write;

pub trait InputStream {
    fn read_byte(&mut self) -> i16;
}

pub trait OutputStream: Send + Sync {
    fn write_byte(&mut self, b: u8);
    fn write_str(&mut self, string: &str);
}

// Implementation of the 'core::fmt::Write' trait for OutputStream
// Required to output formatted strings
// Requires only one function 'write_str'
impl Write for dyn OutputStream {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        Ok(())
    }
}