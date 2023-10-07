#![no_std]

extern crate spin; // we need a mutex in devices::cga_print
extern crate rlibc;

use core::panic::PanicInfo;

// insert other modules
#[macro_use]   // import macros, too
mod devices;
mod kernel;
mod library;
mod consts;

use crate::devices::lfb_terminal;
use crate::kernel::multiboot;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}", info);
    loop {}
}

unsafe fn initialize_lfb(mbi: u64) {
    let fb_info: &multiboot::FrameBufferInfo = multiboot::get_tag(mbi, multiboot::TagType::FramebufferInfo);
    lfb_terminal::initialize(fb_info.addr, fb_info.pitch, fb_info.width, fb_info.height, fb_info.bpp);
}

#[no_mangle]
pub unsafe extern fn startup(mbi: u64) {
    initialize_lfb(mbi);

    println!("Welcome to hhuTOSr!");

    print!("Bootloader: ");
    let mut bootloader_name = multiboot::get_string(mbi, multiboot::TagType::BootLoaderName);
    while bootloader_name.read() != 0 {
        print!("{}", char::from(bootloader_name.read()));
        bootloader_name = bootloader_name.offset(1);
    }
    println!("");
    
    loop{}
}

