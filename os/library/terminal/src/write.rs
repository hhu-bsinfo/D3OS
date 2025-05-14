/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: write                                                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Write a char to the terminal.                                   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 31.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::fmt;
use core::fmt::Write;
use spin::Mutex;
use syscall::{SystemCall, syscall};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::write::print(format_args!($($arg)*));
    });
}

#[macro_export]
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

static WRITER: Mutex<Writer> = Mutex::new(Writer::new());

pub fn print(args: fmt::Arguments) {
    WRITER.lock().write_fmt(args).unwrap();
}

struct Writer {}

impl Writer {
    const fn new() -> Self {
        Self {}
    }
}

pub fn log_debug(s: &str) {
    let res = syscall(
        SystemCall::LogDebug,
        &[s.as_bytes().as_ptr() as usize, s.len()],
    );
    match res {
        Ok(_) => (),
        Err(_) => (),
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let res = syscall(
            SystemCall::TerminalWriteOutput,
            &[s.as_bytes().as_ptr() as usize, s.len()],
        );
        match res {
            Ok(_) => Ok(()),
            Err(_) => Err(fmt::Error),
        }
    }
}
