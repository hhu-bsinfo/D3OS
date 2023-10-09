#![no_std]

extern crate spin; // we need a mutex in devices::cga_print
extern crate rlibc;
extern crate tinyrlibc;
extern crate alloc;

use core::panic::PanicInfo;
use linked_list_allocator::LockedHeap;
use multiboot2::{BootInformation, BootInformationHeader};
use multiboot2::MemoryAreaType::{Available};
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use pc_keyboard::layouts::{AnyLayout, De105Key};
use crate::device::ps2;

// insert other modules
mod device;
mod kernel;
#[macro_use]
mod library;
mod consts;

use crate::library::graphic::lfb_terminal;

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

    // Initialize memory management
    let memory_info = multiboot.memory_map_tag().unwrap();
    let mut heap_area = memory_info.memory_areas().get(0).unwrap();

    for area in memory_info.memory_areas() {
        if area.typ() == Available && area.size() > heap_area.size() {
            heap_area = area;
        }
    }

    // Initialize memory allocation
    ALLOCATOR.lock().init(heap_area.start_address() as *mut u8, (heap_area.end_address() - heap_area.start_address()) as usize);

    // Initialize framebuffer
    let fb_info = multiboot.framebuffer_tag().unwrap().unwrap();
    lfb_terminal::initialize(fb_info.address() as * mut u8, fb_info.pitch(), fb_info.width(), fb_info.height(), fb_info.bpp());

    // Initialize keyboard
    ps2::init_controller();
    ps2::init_keyboard();

    println!("Welcome to hhuTOSr!");
    println!("Bootloader: {}", bootloader_name.name().unwrap());

    let mut controller = ps2::CONTROLLER.lock();
    let mut keyboard = Keyboard::new(ScancodeSet1::new(), AnyLayout::De105Key(De105Key), HandleControl::Ignore);

    loop {
        if let Ok(scancode) = controller.read_data() {
            if let Ok(Some(event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(event) {
                    match key {
                        DecodedKey::Unicode(c) => {
                            print!("{}", c);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

