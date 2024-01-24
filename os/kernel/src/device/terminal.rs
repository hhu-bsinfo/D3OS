use io::stream::{InputStream, OutputStream};
use core::fmt::Write;
use core::ops::Deref;
use core::{fmt, ptr};
use crate::terminal;

pub trait Terminal: OutputStream + InputStream {
    fn clear(&self);
}

// Implementation of the 'core::fmt::Write' trait for our Terminal
// Required to output formatted strings
// Requires only one function 'write_str'
impl Write for dyn Terminal {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.deref().write_str(s);
        return Ok(());
    }
}

// Provide macros like in the 'io' module of Rust
// The $crate variable ensures that the macro also works
// from outside the 'std' crate.
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::device::terminal::print(format_args!($($arg)*));
    });
}

macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

// Helper function of print macros (must be public)
pub fn print(args: fmt::Arguments) {
    // Writing to LFBTerminal does not need a mutable reference,
    // so it is safe to construct a mutable reference here and use it for writing.
    let terminal = unsafe { ptr::from_ref(terminal()).cast_mut().as_mut().unwrap() };
    terminal.write_fmt(args).unwrap();
}
