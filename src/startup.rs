#![no_std]

extern crate spin; // we need a mutex in devices::cga_print
extern crate rlibc;
extern crate tinyrlibc;
extern crate alloc;

use core::panic::PanicInfo;
use linked_list_allocator::LockedHeap;
use multiboot2::{BootInformation, BootInformationHeader};

// insert other modules
#[macro_use]   // import macros, too
mod devices;
mod kernel;
mod library;
mod consts;

use crate::devices::lfb_terminal;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}", info);
    loop {}
}

#[no_mangle]
pub unsafe extern fn startup(mbi: u64) {
    // Get multiboot information
    let multiboot = BootInformation::load(mbi as *const BootInformationHeader).unwrap();
    let bootloader_name = multiboot.boot_loader_name_tag().unwrap();

    // Initialize framebuffer
    let fb_info = multiboot.framebuffer_tag().unwrap().unwrap();
    lfb_terminal::initialize(fb_info.address(), fb_info.pitch(), fb_info.width(), fb_info.height(), fb_info.bpp());

    // Initialize memory allocation
    ALLOCATOR.lock().init(0x300000 as *mut u8, 1024 * 1024);

    println!("Welcome to hhuTOSr!");
    println!("Bootloader: {}", bootloader_name.name().unwrap());
    
    loop{}
}

