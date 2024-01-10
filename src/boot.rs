#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(const_mut_refs)]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(exact_size_is_empty)]
#![feature(panic_info_message)]
#![feature(fmt_internals)]
#![feature(abi_x86_interrupt)]
#![allow(internal_features)]
#![no_main]
#![no_std]

extern crate alloc;
extern crate tinyrlibc;

use crate::kernel::interrupt::interrupt_dispatcher;
use crate::kernel::syscall::syscall_dispatcher;
use crate::kernel::thread::thread::Thread;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ffi::c_void;
use core::fmt::Arguments;
use core::mem::size_of;
use core::ops::Deref;
use core::panic::PanicInfo;
use core::ptr;
use chrono::DateTime;
use log::{debug, error, info, Level, Log, Record};
use multiboot2::{BootInformation, BootInformationHeader, MemoryAreaType, Tag};
use uefi::prelude::*;
use uefi::table::boot::PAGE_SIZE;
use uefi::table::Runtime;
use uefi_raw::table::boot::MemoryType;
use x86_64::instructions::interrupts;
use x86_64::instructions::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::{PhysAddr, VirtAddr};
use x86_64::registers::segmentation::SegmentSelector;
use x86_64::structures::gdt::Descriptor;
use x86_64::structures::paging::{Page, PageTableFlags};
use x86_64::PrivilegeLevel::Ring0;
use x86_64::structures::paging::page::PageRange;
use crate::kernel::memory;
use crate::kernel::memory::MemorySpace;
use crate::kernel::memory::physical::MemoryRegion;

// insert other modules
#[macro_use]
mod device;
mod kernel;
mod library;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if kernel::terminal_initialized() {
        println!("Panic: {}", info);
    } else {
        let record = Record::builder()
            .level(Level::Error)
            .file(Some("panic"))
            .args(*info.message().unwrap_or(&Arguments::new_const(&["A panic occurred!"])))
            .build();

        let logger = kernel::logger().lock();
        unsafe { kernel::logger().force_unlock() }; // log() also calls kernel::logger().lock()
        logger.log(&record);
    }

    loop {}
}

pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

extern "C" {
    static ___KERNEL_DATA_START__: u64;
    static ___KERNEL_DATA_END__: u64;
}

const INIT_HEAP_SIZE: usize = 0x400000;

#[no_mangle]
pub extern "C" fn start(multiboot2_magic: u32, multiboot2_addr: *const BootInformationHeader) {
    // Initialize logger
    if kernel::logger().lock().init().is_err() {
        panic!("Failed to initialize logger!")
    }

    // Log messages and panics are now working, but cannot use format string until the heap is initialized later on
    info!("Welcome to hhuTOSr early boot environment!");

    // Get multiboot information
    if multiboot2_magic != multiboot2::MAGIC {
        panic!("Invalid Multiboot2 magic number!");
    }

    let multiboot = unsafe { BootInformation::load(multiboot2_addr).expect("Failed to get Multiboot2 information!") };

    // Initialize temporary heap, after which format strings may be used in log messages and panics
    let kernel_image_region = kernel_image_region();
    let heap_start = kernel_image_region.end() + 1u64;
    let heap_end = heap_start + (INIT_HEAP_SIZE - 1);

    info!("Initializing temporary heap");
    unsafe { kernel::allocator().init(heap_start.as_u64() as usize, heap_end.as_u64() as usize); }
    debug!("Temporary heap is initialized (Start: [{} MiB], End: [{} MiB]]", heap_start.as_u64() / 1024 / 1024, heap_end.as_u64() / 1024 / 1024);

    let mut bootloader_memory_regions: Vec<MemoryRegion> = Vec::new();

    if let Some(_) = multiboot.efi_bs_not_exited_tag() {
        // EFI boot services have not been exited and we obtain access to the memory map and EFI runtime services by exiting them manually
        info!("EFI boot services have not been exited");
        let image_tag = multiboot.efi_ih64_tag().expect("EFI image handle not available!");
        let sdt_tag = multiboot.efi_sdt64_tag().expect("EFI system table not available!");
        let image_handle;
        let system_table;

        unsafe {
            image_handle = Handle::from_ptr(image_tag.image_handle() as *mut c_void).expect("Failed to create EFI image handle struct from pointer!");
            system_table = SystemTable::<Boot>::from_ptr(sdt_tag.sdt_address() as *mut c_void).expect("Failed to create EFI system table struct from pointer!");
            system_table.boot_services().set_image_handle(image_handle);
        }

        info!("Exiting EFI boot services to obtain runtime system table and memory map");
        let (runtime_table, memory_map) = system_table.exit_boot_services(MemoryType::LOADER_DATA);

        info!("Searching memory map for available regions");
        for area in memory_map.entries() {
            if area.ty == MemoryType::CONVENTIONAL {
                let region = MemoryRegion::from_size(PhysAddr::new(area.phys_start), area.page_count as usize * PAGE_SIZE);
                bootloader_memory_regions.push(region);
            }
        }

        kernel::init_efi_system_table(runtime_table);
    } else if let Some(memory_map) = multiboot.memory_map_tag() {
        // EFI services have been exited, but the bootloader has provided us with a Multiboot2 memory map
        info!("EFI boot services have been exited");
        info!("Bootloader provides Multiboot2 memory map");

        info!("Searching memory map for available regions");
        for area in memory_map.memory_areas() {
            if area.typ() == MemoryAreaType::Available {
                let region = MemoryRegion::new(PhysAddr::new(area.start_address()), PhysAddr::new(area.end_address() - 1));
                bootloader_memory_regions.push(region);
            }
        }

    } else if let Some(memory_map) = multiboot.efi_memory_map_tag() {
        // EFI services have been exited, but the bootloader has provided us with the EFI memory map
        info!("EFI boot services have been exited");
        info!("Bootloader provides EFI memory map");

        info!("Searching memory map for available regions");
        for area in memory_map.memory_areas() {
            if area.ty.0 == MemoryType::CONVENTIONAL.0 {
                let region = MemoryRegion::from_size(PhysAddr::new(area.phys_start), area.page_count as usize * PAGE_SIZE);
                bootloader_memory_regions.push(region);
            }
        }
    } else {
        panic!("No memory information available!");
    }

    // Setup global descriptor table
    // Has to be done after EFI boot services have been exited, since they rely on their own GDT
    info!("Initializing GDT");
    setup_gdt();

    // The bootloader marks the kernel image region as available, so we need to check for regions overlapping
    // with the kernel image and temporary heap and build a new memory map with the kernel image and heap cut out.
    // Furthermore, we need to make sure, that no region start at address 0, ot avoid null pointer panics.
    let mut available_memory_regions = Vec::new();
    let kernel_region = MemoryRegion::new(kernel_image_region.start(), heap_end);

    for mut region in bootloader_memory_regions {
        if region.start() == PhysAddr::zero() {
            if region.end() > PhysAddr::new(memory::PAGE_SIZE as u64) {
                region.set_start(PhysAddr::new(memory::PAGE_SIZE as u64))
            } else {
                continue
            }
        }

        if region.start() < kernel_region.start() && region.end() >= kernel_region.start() { // Region starts below the kernel image
            if region.end() <= kernel_region.end() { // Region starts below and ends inside the kernel image
                available_memory_regions.push(MemoryRegion::new(region.start(), kernel_region.start() - 1u64));
            } else { // Regions starts below and ends above the kernel image
                let lower_region = MemoryRegion::new(region.start(), kernel_region.start() - 1u64);
                let upper_region = MemoryRegion::new(kernel_region.end() + 1u64, region.end());
                available_memory_regions.push(lower_region);
                available_memory_regions.push(upper_region);
            }
        } else if region.start() <= kernel_region.end() && region.end() >= kernel_region.start() { // Region starts within the kernel image
            if region.end() <= kernel_region.end() { // Regions start within and ends within the kernel image
                continue;
            } else { // Region starts within and ends above the kernel image
                available_memory_regions.push(MemoryRegion::new(kernel_region.end() + 1u64, region.end()));
            }
        } else { // Region does not interfere with the kernel image
            available_memory_regions.push(region);
        }
    }

    // Initialize physical memory management
    info!("Initializing page frame allocator");
    unsafe { memory::physical::init(available_memory_regions); }

    // Initialize virtual memory management
    info!("Initializing paging");
    memory::r#virtual::init();

    // Initialize serial port and enable serial logging
    kernel::init_serial_port();
    if let Some(serial) = kernel::serial_port() {
        kernel::logger().lock().register(serial);
    }

    // Initialize terminal and enable terminal logging
    let fb_info = multiboot.framebuffer_tag()
        .expect("No framebuffer information provided by bootloader!")
        .expect("Unknown framebuffer type!");

    let fb_start_page = Page::from_start_address(VirtAddr::new(fb_info.address())).expect("Framebuffer address is not page aligned!");
    let fb_end_page = Page::from_start_address(VirtAddr::new(fb_info.address() + (fb_info.height() * fb_info.pitch()) as u64).align_up(PAGE_SIZE as u64)).unwrap();
    memory::r#virtual::map(PageRange { start: fb_start_page, end: fb_end_page }, MemorySpace::Kernel, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::NO_CACHE);

    kernel::init_terminal(fb_info.address() as *mut u8, fb_info.pitch(), fb_info.width(), fb_info.height(), fb_info.bpp());
    kernel::logger().lock().register(kernel::terminal());

    info!("Welcome to hhuTOSr!");
    let version = format!("v{} ({} - O{})", built_info::PKG_VERSION, built_info::PROFILE, built_info::OPT_LEVEL);
    let git_ref = built_info::GIT_HEAD_REF.unwrap_or("Unknown");
    let git_commit = built_info::GIT_COMMIT_HASH_SHORT.unwrap_or("Unknown");
    let build_date = match DateTime::parse_from_rfc2822(built_info::BUILT_TIME_UTC) {
        Ok(date_time) => date_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        Err(_) => "Unknown".to_string(),
    };
    let bootloader_name = match multiboot.boot_loader_name_tag() {
        Some(tag) => if tag.name().is_ok() { tag.name().unwrap_or("Unknown") } else { "Unknown" },
        None => "Unknown",
    };

    info!("OS Version: [{}]", version);
    info!("Git Version: [{} - {}]", built_info::GIT_HEAD_REF.unwrap_or_else(|| "Unknown"), git_commit);
    info!("Build Date: [{}]", build_date);
    info!("Compiler: [{}]", built_info::RUSTC_VERSION);
    info!("Bootloader: [{}]", bootloader_name);

    // Initialize ACPI tables
    let rsdp_addr: usize = if let Some(rsdp_tag) = multiboot.rsdp_v2_tag() {
        ptr::from_ref(rsdp_tag) as usize + size_of::<Tag>()
    } else if let Some(rsdp_tag) = multiboot.rsdp_v1_tag() {
        ptr::from_ref(rsdp_tag) as usize + size_of::<Tag>()
    } else {
        panic!("ACPI not available!");
    };

    kernel::init_acpi_tables(rsdp_addr);

    // Initialize interrupts
    info!("Initializing IDT");
    interrupt_dispatcher::setup_idt();
    info!("Initializing system calls");
    syscall_dispatcher::init();
    kernel::init_apic();

    // Initialize timer
    {
        info!("Initializing timer");
        let mut timer = kernel::timer().write();
        timer.interrupt_rate(1);
        timer.plugin();
    }

    // Enable interrupts
    info!("Enabling interrupts");
    interrupts::enable();

    // Initialize EFI runtime service (if available and not done already during memory initialization)
    if kernel::efi_system_table().is_none() {
        if let Some(sdt_tag) = multiboot.efi_sdt64_tag() {
            info!("Initializing EFI runtime services");
            let system_table = unsafe { SystemTable::<Runtime>::from_ptr(sdt_tag.sdt_address() as *mut c_void) };
            if system_table.is_some() {
                kernel::init_efi_system_table(system_table.unwrap());
            } else {
                error!("Failed to create EFI system table struct from pointer!");
            }
        }
    }

    if let Some(system_table) = kernel::efi_system_table() {
        info!("EFI runtime services available (Vendor: [{}], UEFI version: [{}])", system_table.firmware_vendor(), system_table.uefi_revision());
    }

    // Initialize keyboard
    info!("Initializing PS/2 devices");
    kernel::init_keyboard();
    kernel::ps2_devices().keyboard().plugin();

    // Enable serial port interrupts
    if let Some(serial) = kernel::serial_port() {
        serial.plugin();
    }

    let scheduler = kernel::scheduler();
    scheduler.ready(Thread::new_kernel_thread(Box::new(|| {
        let terminal = kernel::terminal();
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
    kernel::logger().lock().remove(kernel::terminal());
    kernel::terminal().clear();

    println!(include_str!("banner.txt"), version, git_ref.rsplit("/").next().unwrap_or(git_ref), git_commit, build_date,
             built_info::RUSTC_VERSION.split_once("(").unwrap_or((built_info::RUSTC_VERSION, "")).0.trim(), bootloader_name);

    info!("Starting scheduler");
    scheduler.start();
}

fn kernel_image_region() -> MemoryRegion {
    let start: PhysAddr;
    let mut end: PhysAddr;

    unsafe {
        start = PhysAddr::new(ptr::from_ref(&___KERNEL_DATA_START__) as u64);
        end = PhysAddr::new(ptr::from_ref(&___KERNEL_DATA_END__) as u64);
    }

    // Align up to 1 MiB
    end = end.align_up(0x100000u64);

    return MemoryRegion::new(start, end);
}

fn setup_gdt() {
    let mut gdt = kernel::gdt().lock();
    let tss = kernel::tss().lock();

    gdt.add_entry(Descriptor::kernel_code_segment());
    gdt.add_entry(Descriptor::kernel_data_segment());
    gdt.add_entry(Descriptor::user_data_segment());
    gdt.add_entry(Descriptor::user_code_segment());

    unsafe {
        // We need to obtain a static reference to the TSS and GDT for the following operations.
        // We know, that they have a static lifetime, since they are declared as static variables in 'kernel/mod.rs'.
        // However, since they are hidden behind a Mutex, the borrow checker does not see them with a static lifetime.
        let gdt_ref = ptr::from_ref(gdt.deref()).as_ref().unwrap();
        let tss_ref = ptr::from_ref(tss.deref()).as_ref().unwrap();
        gdt.add_entry(Descriptor::tss_segment(tss_ref));
        gdt_ref.load();
    }

    unsafe {
        // Load task state segment
        load_tss(SegmentSelector::new(5, Ring0));

        // Set code and stack segment register
        CS::set_reg(SegmentSelector::new(1, Ring0));
        SS::set_reg(SegmentSelector::new(2, Ring0));

        // Other segment registers are not used in long mode (set to 0)
        DS::set_reg(SegmentSelector::new(0, Ring0));
        ES::set_reg(SegmentSelector::new(0, Ring0));
        FS::set_reg(SegmentSelector::new(0, Ring0));
        GS::set_reg(SegmentSelector::new(0, Ring0));
    }
}
