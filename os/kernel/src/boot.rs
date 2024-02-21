use alloc::boxed::Box;
use crate::interrupt::interrupt_dispatcher;
use crate::syscall::syscall_dispatcher;
use crate::process::thread::Thread;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use core::ffi::c_void;
use core::mem::size_of;
use core::ops::Deref;
use core::ptr;
use chrono::DateTime;
use log::{debug, error, info};
use multiboot2::{BootInformation, BootInformationHeader, EFIMemoryMapTag, MemoryAreaType, MemoryMapTag, Tag};
use uefi::prelude::*;
use uefi::table::boot::{MemoryMap, PAGE_SIZE};
use uefi::table::Runtime;
use uefi_raw::table::boot::MemoryType;
use x86_64::instructions::interrupts;
use x86_64::instructions::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::{PhysAddr, VirtAddr};
use x86_64::registers::segmentation::SegmentSelector;
use x86_64::structures::gdt::Descriptor;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};
use x86_64::PrivilegeLevel::Ring0;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use crate::{allocator, apic, built_info, efi_system_table, gdt, init_acpi_tables, init_apic, init_efi_system_table, init_initrd, init_keyboard, init_pci, init_serial_port, init_terminal, initrd, logger, memory, ps2_devices, scheduler, serial_port, terminal, timer, tss};
use crate::memory::MemorySpace;
use crate::process::process::create_process;

extern "C" {
    static ___KERNEL_DATA_START__: u64;
    static ___KERNEL_DATA_END__: u64;
}

const INIT_HEAP_PAGES: usize = 0x400;

#[no_mangle]
pub extern "C" fn start(multiboot2_magic: u32, multiboot2_addr: *const BootInformationHeader) {
    // Initialize logger
    if logger().lock().init().is_err() {
        panic!("Failed to initialize loggerr!")
    }

    // Log messages and panics are now working, but cannot use format string until the heap is initialized later on
    info!("Welcome to D3OS early boot environment!");

    // Get multiboot information
    if multiboot2_magic != multiboot2::MAGIC {
        panic!("Invalid Multiboot2 magic number!");
    }

    let multiboot = unsafe { BootInformation::load(multiboot2_addr).expect("Failed to get Multiboot2 information!") };

    // Search memory map, provided by bootloader of EFI, for usable memory and initialize physical memory management
    if let Some(_) = multiboot.efi_bs_not_exited_tag() {
        // EFI boot services have not been exited, and we obtain access to the memory map and EFI runtime services by exiting them manually
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

        scan_efi_memory_map(&memory_map);
        init_efi_system_table(runtime_table);
    } else {
        info!("EFI boot services have been exited");
        if let Some(memory_map) = multiboot.efi_memory_map_tag() {
            // EFI services have been exited, but the bootloader has provided us with the EFI memory map
            info!("Bootloader provides EFI memory map");
            scan_efi_multiboot2_memory_map(memory_map);
        } else if let Some(memory_map) = multiboot.memory_map_tag() {
            // EFI services have been exited, but the bootloader has provided us with a Multiboot2 memory map
            info!("Bootloader provides Multiboot2 memory map");
            scan_multiboot2_memory_map(memory_map);
        } else {
            panic!("No memory information available!");
        }
    }

    // Setup global descriptor table
    // Has to be done after EFI boot services have been exited, since they rely on their own GDT
    info!("Initializing GDT");
    init_gdt();

    // The bootloader marks the kernel image region as available, so we need to reserve it manually
    unsafe { memory::physical::reserve(kernel_image_region()); }

    // and initialize kernel heap, after which format strings may be used in logs and panics.
    info!("Initializing kernel heap");
    let heap_region = memory::physical::alloc(INIT_HEAP_PAGES);
    unsafe { allocator().init(&heap_region); }
    debug!("Kernel heap is initialized [0x{:x} - 0x{:x}]", heap_region.start.start_address().as_u64(), heap_region.end.start_address().as_u64());
    debug!("Page frame allocator:\n{}", memory::physical::dump());

    // Initialize virtual memory management
    info!("Initializing paging");
    let kernel_process = create_process();
    kernel_process.address_space().load();

    // Initialize serial port and enable serial logging
    init_serial_port();
    if let Some(serial) = serial_port() {
        logger().lock().register(serial);
    }

    // Initialize terminal and enable terminal logging
    let fb_info = multiboot.framebuffer_tag()
        .expect("No framebuffer information provided by bootloader!")
        .expect("Unknown framebuffer type!");

    let fb_start_page = Page::from_start_address(VirtAddr::new(fb_info.address())).expect("Framebuffer address is not page aligned!");
    let fb_end_page = Page::from_start_address(VirtAddr::new(fb_info.address() + (fb_info.height() * fb_info.pitch()) as u64).align_up(PAGE_SIZE as u64)).unwrap();
    kernel_process.address_space().map(PageRange { start: fb_start_page, end: fb_end_page }, MemorySpace::Kernel, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);

    init_terminal(fb_info.address() as *mut u8, fb_info.pitch(), fb_info.width(), fb_info.height(), fb_info.bpp());
    logger().lock().register(terminal());

    info!("Welcome to D3OS!");
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

    init_acpi_tables(rsdp_addr);

    // Initialize interrupts
    info!("Initializing IDT");
    interrupt_dispatcher::setup_idt();
    info!("Initializing system calls");
    syscall_dispatcher::init();
    init_apic();

    // Initialize timer
    {
        info!("Initializing timer");
        let mut timer = timer().write();
        timer.interrupt_rate(1);
        timer.plugin();
    }

    // Enable interrupts
    info!("Enabling interrupts");
    interrupts::enable();

    // Initialize EFI runtime service (if available and not done already during memory initialization)
    if efi_system_table().is_none() {
        if let Some(sdt_tag) = multiboot.efi_sdt64_tag() {
            info!("Initializing EFI runtime services");
            let system_table = unsafe { SystemTable::<Runtime>::from_ptr(sdt_tag.sdt_address() as *mut c_void) };
            if system_table.is_some() {
                init_efi_system_table(system_table.unwrap());
            } else {
                error!("Failed to create EFI system table struct from pointer!");
            }
        }
    }

    if let Some(system_table) = efi_system_table() {
        info!("EFI runtime services available (Vendor: [{}], UEFI version: [{}])", system_table.firmware_vendor(), system_table.uefi_revision());
    }

    // Initialize keyboard
    info!("Initializing PS/2 devices");
    init_keyboard();
    ps2_devices().keyboard().plugin();

    // Enable serial port interrupts
    if let Some(serial) = serial_port() {
        serial.plugin();
    }

    // Scan PCI bus
    init_pci();

    // Load initial ramdisk
    let initrd_tag = multiboot.module_tags()
        .find(|module| module.cmdline().is_ok_and(|name| name == "initrd"))
        .expect("Initrd not found!");
    init_initrd(initrd_tag);

    // Ready terminal read thread
    scheduler().ready(Thread::new_kernel_thread(Box::new(|| {
        let mut command = String::new();
        let terminal = terminal();
        terminal.write_str("> ");

        loop {
            match terminal.read_byte() {
                -1 => panic!("Terminal input stream closed!"),
                0x0a => {
                    match initrd().entries().find(|entry| entry.filename().as_str() == command) {
                        Some(app) => {
                            let thread = Thread::new_user_thread(app.data());
                            scheduler().ready(Rc::clone(&thread));
                            thread.join();
                        }
                        None => {
                            if !command.is_empty() {
                                println!("Command not found!");
                            }
                        }
                    }

                    command.clear();
                    terminal.write_str("> ")
                },
                c => command.push(char::from_u32(c as u32).unwrap())
            }
        }
    })));

    // Ready shell thread
    /*scheduler().ready(Thread::new_user_thread(initrd().entries()
        .find(|entry| entry.filename().as_str() == "shell")
        .expect("Shell application not available!")
        .data()));*/

    // Disable terminal logging
    logger().lock().remove(terminal());
    terminal().clear();

    println!(include_str!("banner.txt"), version, git_ref.rsplit("/").next().unwrap_or(git_ref), git_commit, build_date,
             built_info::RUSTC_VERSION.split_once("(").unwrap_or((built_info::RUSTC_VERSION, "")).0.trim(), bootloader_name);

    info!("Starting scheduler");
    apic().start_timer(10);
    scheduler().start();
}

fn init_gdt() {
    let mut gdt = gdt().lock();
    let tss = tss().lock();

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

fn kernel_image_region() -> PhysFrameRange {
    let start: PhysFrame;
    let end: PhysFrame;

    unsafe {
        start = PhysFrame::from_start_address(PhysAddr::new(ptr::from_ref(&___KERNEL_DATA_START__) as u64)).expect("Kernel code is not page aligned!");
        end = PhysFrame::from_start_address(PhysAddr::new(ptr::from_ref(&___KERNEL_DATA_END__) as u64).align_up(PAGE_SIZE as u64)).unwrap();
    }

    return PhysFrameRange { start, end };
}

fn scan_efi_memory_map(memory_map: &MemoryMap) {
    info!("Searching memory map for available regions");
    memory_map.entries()
        .filter(|area| area.ty == MemoryType::CONVENTIONAL || area.ty == MemoryType::LOADER_CODE || area.ty == MemoryType::LOADER_DATA
            || area.ty == MemoryType::BOOT_SERVICES_CODE || area.ty == MemoryType::BOOT_SERVICES_DATA)
        .for_each(|area| {
            let start = PhysFrame::from_start_address(PhysAddr::new(area.phys_start).align_up(PAGE_SIZE as u64)).unwrap();
            unsafe { memory::physical::insert(PhysFrameRange { start, end: start + area.page_count }); }
        });
}

fn scan_efi_multiboot2_memory_map(memory_map: &EFIMemoryMapTag) {
    info!("Searching memory map for available regions");
    memory_map.memory_areas()
        .filter(|area| area.ty.0 == MemoryType::CONVENTIONAL.0 || area.ty.0 == MemoryType::LOADER_CODE.0 || area.ty.0 == MemoryType::LOADER_DATA.0
            || area.ty.0 == MemoryType::BOOT_SERVICES_CODE.0 || area.ty.0 == MemoryType::BOOT_SERVICES_DATA.0) // .0 necessary because of different version dependencies to uefi-crate
        .for_each(|area| {
            let start = PhysFrame::from_start_address(PhysAddr::new(area.phys_start).align_up(PAGE_SIZE as u64)).unwrap();
            unsafe { memory::physical::insert(PhysFrameRange { start, end: start + area.page_count }); }
        });
}

fn scan_multiboot2_memory_map(memory_map: &MemoryMapTag) {
    info!("Searching memory map for available regions");
    memory_map.memory_areas().iter()
        .filter(|area| area.typ() == MemoryAreaType::Available)
        .for_each(|area| {
            unsafe {
                memory::physical::insert(PhysFrameRange {
                    start: PhysFrame::from_start_address(PhysAddr::new(area.start_address()).align_up(PAGE_SIZE as u64)).unwrap(),
                    end: PhysFrame::from_start_address(PhysAddr::new(area.end_address()).align_down(PAGE_SIZE as u64)).unwrap()
                });
            }
        });
}