#![feature(ptr_internals)]
#![feature(const_mut_refs)]
#![no_std]

extern crate spin; // we need a mutex in devices::cga_print
extern crate rlibc;

use core::panic::PanicInfo;

// insert other modules
#[macro_use]   // import macros, too
mod devices;
mod kernel;
mod user;
mod consts;

use crate::devices::terminal;   // used to import code needed by println!

use user::aufgabe1::text_demo;
use user::aufgabe1::keyboard_demo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}", info);
    loop {}
}

unsafe fn initialize_lfb(mbi: u64) {
    let flags = (mbi as *mut u32).read();
    if flags & 0x1000 == 0 {
        panic!("System is not using a graphics mode!");
    }

    let addr = ((mbi + 88) as *mut u64).read();
    let pitch = ((mbi + 96) as *mut u32).read();
    let width = ((mbi + 100) as *mut u32).read();
    let height = ((mbi + 104) as *mut u32).read();
    let bpp = ((mbi + 108) as *mut u8).read();

    terminal::initialize(addr, pitch, width, height, bpp);
}

fn aufgabe1() {
   text_demo::run();
   keyboard_demo::run();
}


#[no_mangle]
pub extern fn startup(mbi: u64) {
    unsafe { initialize_lfb(mbi); }

    println!("Welcome to hhuTOSr!");

    aufgabe1();
    
    loop{}
}

