#![feature(ptr_from_ref)]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(trait_upcasting)]
#![feature(const_for)]
#![feature(new_uninit)]
#![feature(const_mut_refs)]
#![feature(naked_functions)]
#![no_std]

extern crate spin; // we need a mutex in devices::cga_print
extern crate rlibc;
extern crate tinyrlibc;
extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::ToString;
use core::mem::size_of;
use core::panic::PanicInfo;
use core::ptr;
use chrono::DateTime;
use lazy_static::lazy_static;
use multiboot2::{BootInformation, BootInformationHeader, Tag};
use multiboot2::MemoryAreaType::{Available};
use x86_64::instructions::interrupts;
use crate::kernel::log::Logger;
use crate::kernel::thread::thread::Thread;

// insert other modules
#[macro_use]
mod device;
mod kernel;
mod library;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}", info);
    loop {}
}

pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

lazy_static! {
static ref LOG: Logger = Logger::new("Boot");
}

#[no_mangle]
pub unsafe extern fn startup(mbi: u64) {
    // Get multiboot information
    let multiboot = BootInformation::load(mbi as *const BootInformationHeader).unwrap();

    // Initialize memory management
    let memory_info = multiboot.memory_map_tag().unwrap();
    let mut heap_area = memory_info.memory_areas().get(0).unwrap();

    for area in memory_info.memory_areas() {
        if area.typ() == Available && area.size() > heap_area.size() {
            heap_area = area;
        }
    }

    kernel::get_memory_service().init(heap_area.start_address() as usize, heap_area.end_address() as usize);

    // Initialize scheduler
    kernel::get_thread_service().init();

    // Initialize serial port and enable serial logging
    kernel::get_device_service().init_serial_port();
    match kernel::get_device_service().get_serial_port() {
        Some(serial) => {
            kernel::get_log_service().register(serial);
        }
        None => {}
    }

    // Initialize terminal and enable terminal logging
    let fb_info = multiboot.framebuffer_tag().unwrap().unwrap();
    kernel::get_device_service().init_terminal(fb_info.address() as *mut u8, fb_info.pitch(), fb_info.width(), fb_info.height(), fb_info.bpp());
    kernel::get_log_service().register(kernel::get_device_service().get_terminal());

    LOG.info("Welcome to hhuTOSr!");
    LOG.info("Memory management has been initialized!");

    // Initialize ACPI tables
    let rsdp_addr: usize = if let Some(rsdp_tag) = multiboot.rsdp_v2_tag() {
        ptr::from_ref(rsdp_tag) as usize + size_of::<Tag>()
    } else if let Some(rsdp_tag) = multiboot.rsdp_v1_tag() {
        ptr::from_ref(rsdp_tag) as usize + size_of::<Tag>()
    } else {
        panic!("ACPI not available!");
    };

    kernel::get_device_service().init_acpi_tables(rsdp_addr);
    LOG.info(format!("ACPI revision: [{}]", kernel::get_device_service().get_acpi_tables().revision()).as_str());

    // Initialize interrupts
    kernel::get_interrupt_service().init();
    LOG.info("Enabling interrupts");
    interrupts::enable();

    // Initialize timer
    LOG.info("Initializing timer");
    kernel::get_time_service().init();

    // Initialize keyboard
    LOG.info("Initializing PS/2 devices");
    kernel::get_device_service().init_keyboard();

    // Enable serial port interrupts
    match kernel::get_device_service().get_serial_port() {
        Some(serial) => {
            serial.plugin();
        }
        None => {}
    }

    let thread_service = kernel::get_thread_service();
    thread_service.ready_thread(Thread::new_kernel_thread(Box::new(|| {
        let terminal = kernel::get_device_service().get_terminal();
        terminal.write_str("> ");

        loop {
            match terminal.read_byte() {
                -1 => panic!("Terminal input stream closed!"),
                0x0a => terminal.write_str("> "),
                _ => {}
            }
        }
    })));

    // Disable terminal logging
    kernel::get_log_service().remove(kernel::get_device_service().get_terminal());
    kernel::get_device_service().get_terminal().clear();

    let version = format!("v{} ({})", built_info::PKG_VERSION, built_info::PROFILE);
    let date = match DateTime::parse_from_rfc2822(built_info::BUILT_TIME_UTC) {
        Ok(date_time) => date_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        Err(_) => "Unknown".to_string()
    };
    let git_ref = match built_info::GIT_HEAD_REF {
        Some(str) => str,
        None => "Unknown"
    };
    let git_commit = match built_info::GIT_COMMIT_HASH_SHORT {
        Some(str) => str,
        None => "Unknown"
    };
    let bootloader_name = match multiboot.boot_loader_name_tag() {
        Some(tag) => if tag.name().is_ok() { tag.name().unwrap() } else { "Unknown" },
        None => "Unknown"
    };

    println!(include_str!("banner.txt"), version, date, git_ref, git_commit, bootloader_name);

    LOG.info("Starting scheduler");
    thread_service.start_scheduler();
}

