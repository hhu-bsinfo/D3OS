#![no_std]

use core::panic::PanicInfo;
use io::{print, println};

extern {
    fn main();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}!", info);
    thread::exit();
}

#[no_mangle]
extern "C" fn entry() {
    unsafe { main(); }
    thread::exit();
}