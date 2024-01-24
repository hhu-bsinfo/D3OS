#![no_std]

use core::fmt;
use core::fmt::Write;
use core::panic::PanicInfo;
use spin::Mutex;
use syscall::{syscall1, SystemCall};

macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::print(format_args!($($arg)*));
    });
}

macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

struct Writer {}

impl Writer {
    const fn new() -> Self {
        Self {}
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            syscall1(SystemCall::Print as u64, c as u64);
        }

        return Ok(());
    }
}

pub fn print(args: fmt::Arguments) {
    WRITER.lock().write_fmt(args).unwrap();
}

static WRITER: Mutex<Writer> = Mutex::new(Writer::new());

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}!", info);
    loop {}
}

#[no_mangle]
pub extern "C" fn main() {
    println!("Hello, world!");
    thread::exit();
}