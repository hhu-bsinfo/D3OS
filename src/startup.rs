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
use crate::kernel::multiboot;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}", info);
    loop {}
}

fn initialize_lfb(mbi: u64) {
    let fb_info: multiboot::FrameBufferInfo = multiboot::get_tag(mbi, multiboot::TagType::FramebufferInfo);
    terminal::initialize(fb_info.addr, fb_info.pitch, fb_info.width, fb_info.height, fb_info.bpp);
}

fn aufgabe1() {
   text_demo::run();
   keyboard_demo::run();
}


#[no_mangle]
pub extern fn startup(mbi: u64) {
    initialize_lfb(mbi);

    println!("Welcome to hhuTOSr!");

    aufgabe1();
    
    loop{}
}

