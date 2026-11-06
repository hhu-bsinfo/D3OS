/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: write                                                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Write output to the terminal.                                   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Authors: Fabian Ruhland, 31.8.2024, HHU,                                ║
   ║          Aymen Sellami                                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::fmt;
use core::fmt::Write;
use naming::shared_types::OpenOptions;
use spin::Mutex;

use crate::session::Session;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::write::print(format_args!($($arg)*));
    });
}

#[macro_export]
macro_rules! println {
    ($fmt:expr) => ({
        $crate::write::print(format_args!(concat!($fmt, "\n")));
    });
    ($fmt:expr, $($arg:tt)*) => ({
        $crate::write::print(format_args!(concat!($fmt, "\n"), $($arg)*));
    });
}

pub static TERMINAL_WRITER: Mutex<Writer> = Mutex::new(Writer::new());

pub fn print(args: fmt::Arguments) {
    TERMINAL_WRITER.lock().write_fmt(args).unwrap();
}

/// Write raw bytes to the session stdout (`out` FIFO). Uses the same stdout stream
/// as print!/println!. Intended for non-Rust callers (e.g. the C runtime) that
/// already hold encoded bytes. Returns `true` on success.
pub fn write_bytes(bytes: &[u8]) -> bool {
    let mut writer = TERMINAL_WRITER.lock();
    match writer.handle() {
        Some(handle) => naming::write(handle, bytes).is_ok(),
        None => false,
    }
}

pub struct Writer {
    /// Lazily opened handle to the session `out` FIFO (stdout). Opening is
    /// deferred to the first write so processes that never print do not block
    /// on the pipe rendezvous.
    out: Option<usize>,
}

impl Writer {
    const fn new() -> Self {
        Self { out: None }
    }

    fn handle(&mut self) -> Option<usize> {
        if self.out.is_none() {
            // Open once and keep the handle for the process lifetime
            // so stdout is not reopened between writes
            self.out = naming::open(&Session::current().out_path(), OpenOptions::WRITEONLY).ok();
        }
        self.out
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // core::fmt::Write may call write_str(""), sys_write rejects
        // zero-length buffers with EINVAL, so we handle a no-op here
        if s.is_empty() {
            return Ok(());
        }
        let handle = self.handle().ok_or(fmt::Error)?;
        match naming::write(handle, s.as_bytes()) {
            Ok(_) => Ok(()),
            Err(_) => Err(fmt::Error),
        }
    }
}
