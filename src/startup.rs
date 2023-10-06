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

use devices::cga;         // shortcut for cga
use devices::cga_print;   // used to import code needed by println!

use user::aufgabe1::text_demo;
use user::aufgabe1::keyboard_demo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}", info);
    loop {}
}

fn aufgabe1() {
   text_demo::run();
   keyboard_demo::run();
}


#[no_mangle]
pub extern fn startup() {
    cga::clear();
    println!("Welcome to hhuTOSr!");

    aufgabe1();
    
    loop{}
}

