use crate::terminal;
use core::fmt::Write;
use core::ops::Deref;
use core::{fmt, ptr};
use alloc::string::String;
use stream::{InputStream, OutputStream};
use syscall::{SystemCall, syscall};

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

pub fn log_debug(args: fmt::Arguments) {
    let mut log_message = String::new();
    log_message.write_fmt(args).unwrap();

    syscall(SystemCall::LogDebug, &[
        log_message.as_bytes().as_ptr() as usize,
        log_message.len(),
    ])
    .expect("Failed to log debug message!");
}

// Helper function of print macros (must be public)
pub fn print(args: fmt::Arguments) {
    let terminal = Some(terminal());

    match terminal {
        Some(terminal) => {
            // Writing to LFBTerminal does not need a mutable reference,
            // so it is safe to construct a mutable reference here and use it for writing.
            let terminal_mut = unsafe {
                ptr::from_ref(terminal.as_ref())
                    .cast_mut()
                    .as_mut()
                    .unwrap()
            };
            terminal_mut.write_fmt(args).unwrap();
        }
        None => {
            // If the terminal is not yet initialized, print to the serial port
            log_debug(args);
        }
    }

    // // Writing to LFBTerminal does not need a mutable reference,
    // // so it is safe to construct a mutable reference here and use it for writing.
    // let terminal_mut = unsafe {
    //     ptr::from_ref(terminal.as_ref())
    //         .cast_mut()
    //         .as_mut()
    //         .unwrap()
    // };
    // terminal_mut.write_fmt(args).unwrap();
}
