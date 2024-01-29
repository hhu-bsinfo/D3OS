#![no_std]

use core::fmt;
use core::fmt::Write;
use core::panic::PanicInfo;
use spin::Mutex;
use syscall::{syscall2, SystemCall};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::print(format_args!($($arg)*));
    });
}

#[macro_export]
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

extern {
    fn main();
}

static WRITER: Mutex<Writer> = Mutex::new(Writer::new());

struct Writer {}

impl Writer {
    const fn new() -> Self {
        Self {}
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        syscall2(SystemCall::Write as u64, s.as_bytes().as_ptr() as u64, s.len() as u64);
        return Ok(());
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}!", info);
    thread::exit();
}

pub fn print(args: fmt::Arguments) {
    WRITER.lock().write_fmt(args).unwrap();
}

#[no_mangle]
extern "C" fn entry() {
    unsafe { main(); }
    thread::exit();
}