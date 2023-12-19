#![feature(ptr_from_ref)]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(trait_upcasting)]
#![feature(const_for)]
#![feature(new_uninit)]
#![feature(const_mut_refs)]
#![feature(naked_functions)]
#![feature(exact_size_is_empty)]
#![no_main]
#![no_std]

extern crate spin; // we need a mutex in devices::cga_print
extern crate rlibc;
extern crate tinyrlibc;
extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::ToString;
use core::arch::asm;
use core::ffi::c_void;
use core::mem::size_of;
use core::panic::PanicInfo;
use core::ptr;
use chrono::DateTime;
use lazy_static::lazy_static;
use multiboot2::{BootInformation, BootInformationHeader, MemoryAreaType, Tag};
use uefi_raw::table::boot::MemoryType;
use x86_64::instructions::interrupts;
use uefi::prelude::*;
use uefi::table::boot::PAGE_SIZE;
use uefi::table::Runtime;
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

extern "C" {
    static ___BSS_START__: u64;
    static ___BSS_END__: u64;
}

lazy_static! {
static ref LOG: Logger = Logger::new("Boot");
}

#[no_mangle]
pub unsafe extern fn start() {
    // Disable interrupts and get multiboot2 structure address from ebx
    interrupts::disable();
    let mbi = get_mbi();

    clear_bss();

    // Get multiboot information
    let multiboot = BootInformation::load(mbi as *const BootInformationHeader).unwrap();

    let heap_start: usize;
    let heap_end: usize;

    if let Some(_) = multiboot.efi_bs_not_exited_tag() {
        // EFI services have not been exited and we obtain access to the memory map and EFI runtime services by exiting them manually
        let image_tag = multiboot.efi_ih64_tag().unwrap_or_else(|| panic!("EFI image handle not available!"));
        let sdt_tag = multiboot.efi_sdt64_tag().unwrap_or_else(|| panic!("EFI system table not available!"));
        let image_handle = Handle::from_ptr(image_tag.image_handle() as *mut c_void).unwrap_or_else(|| panic!("Failed to create EFI image handle struct from pointer!"));
        let system_table = SystemTable::<Boot>::from_ptr(sdt_tag.sdt_address() as *mut c_void).unwrap_or_else(|| panic!("Failed to create EFI system table struct from pointer!"));

        system_table.boot_services().set_image_handle(image_handle);
        let (runtime_table, memory_map) = system_table.exit_boot_services(MemoryType::LOADER_DATA);

        let mut heap_area = memory_map.entries().next().unwrap();
        for area in memory_map.entries() {
            if area.ty == MemoryType::CONVENTIONAL && area.page_count > heap_area.page_count {
                heap_area = area;
            }
        }

        heap_start = heap_area.phys_start as usize;
        heap_end = heap_area.phys_start as usize + heap_area.page_count as usize * PAGE_SIZE - 1;

        kernel::set_efi_system_table(runtime_table);
    } else if let Some(memory_map) = multiboot.memory_map_tag() {
        // EFI services have been exited, but the bootloader has provided us with a memory map
        let mut heap_area = memory_map.memory_areas().get(0).unwrap_or_else(|| panic!("Multiboot2 memory map is empty!"));

        for area in memory_map.memory_areas() {
            if area.typ() == MemoryAreaType::Available && area.size() > heap_area.size() {
                heap_area = area;
            }
        }

        heap_start = heap_area.start_address() as usize;
        heap_end = heap_area.end_address() as usize;
    } else if let Some (memory_map) = multiboot.efi_memory_map_tag() {
        // EFI services have been exited, but the bootloader has provided us with the EFI memory map
        let mut heap_area = memory_map.memory_areas().next().unwrap_or_else(|| panic!("EFI memory map is empty!"));

        for area in memory_map.memory_areas() {
            if area.ty.0 == MemoryType::CONVENTIONAL.0 && area.page_count > heap_area.page_count {
                heap_area = area;
            }
        }

        heap_start = heap_area.phys_start as usize;
        heap_end = (heap_area.phys_start + heap_area.page_count * 4096 - 1) as usize;
    } else {
        panic!("No memory information available!");
    }

    kernel::get_memory_service().init(heap_start, heap_end);

    // Initialize thread service, which sets up GDT and TSS
    let thread_service = kernel::get_thread_service();
    thread_service.init();

    // Initialize ACPI tables
    let rsdp_addr: usize = if let Some(rsdp_tag) = multiboot.rsdp_v2_tag() {
        ptr::from_ref(rsdp_tag) as usize + size_of::<Tag>()
    } else if let Some(rsdp_tag) = multiboot.rsdp_v1_tag() {
        ptr::from_ref(rsdp_tag) as usize + size_of::<Tag>()
    } else {
        panic!("ACPI not available!");
    };

    kernel::get_device_service().init_acpi_tables(rsdp_addr);

    // Initialize interrupts
    kernel::get_interrupt_service().init();
    interrupts::enable();

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
    LOG.info(format!("Heap area: {} MiB - {} MiB", heap_start / 1024 / 1024, heap_end / 1024 / 1024).as_str());

    // Initialize timer
    LOG.info("Initializing timer");
    kernel::get_time_service().init();

    // Initialize EFI runtime service (if available and not done already during memory initialization)
    if kernel::get_efi_system_table().is_none() {
        if let Some(sdt_tag) = multiboot.efi_sdt64_tag() {
            LOG.info("Initializing EFI runtime services");
            let system_table = SystemTable::<Runtime>::from_ptr(sdt_tag.sdt_address() as *mut c_void);

            if system_table.is_some() {
                kernel::set_efi_system_table(system_table.unwrap());
            } else {
                LOG.error("Failed to create EFI system table struct from pointer!");
            }
        }
    }

    if let Some(system_table) = kernel::get_efi_system_table() {
        LOG.info(format!("EFI runtime services available (Vendor: [{}], UEFI version: [{}])", system_table.firmware_vendor(), system_table.uefi_revision()).as_str());
    }

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
    let git_ref = built_info::GIT_HEAD_REF.unwrap_or_else(|| "Unknown");
    let git_commit = built_info::GIT_COMMIT_HASH_SHORT.unwrap_or_else(|| "Unknown");
    let date = match DateTime::parse_from_rfc2822(built_info::BUILT_TIME_UTC) {
        Ok(date_time) => date_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        Err(_) => "Unknown".to_string()
    };
    let bootloader_name = match multiboot.boot_loader_name_tag() {
        Some(tag) => if tag.name().is_ok() { tag.name().unwrap() } else { "Unknown" },
        None => "Unknown"
    };

    println!(include_str!("banner.txt"), version, date, git_ref, git_commit, bootloader_name);

    LOG.info("Starting scheduler");
    thread_service.start_scheduler();
}

unsafe fn get_mbi() -> u32 {
    let multiboot2_magic: u32;
    let multiboot2_address: u32;

    asm!(
    "",
    out("eax") multiboot2_magic,
    );

    asm!(
    "mov {:e}, ebx",
    out(reg) multiboot2_address
    );

    if multiboot2_magic != multiboot2::MAGIC {
        panic!("Invalid Multiboot2 magic number [{}]!", multiboot2_magic);
    }

    return multiboot2_address;
}

unsafe fn clear_bss() {
    let bss_start = ptr::from_ref(&___BSS_START__) as *mut u8;
    let bss_end = ptr::from_ref(&___BSS_END__) as *const u8;
    let length = bss_end as usize - bss_start as usize;

    bss_start.write_bytes(0, length);
}