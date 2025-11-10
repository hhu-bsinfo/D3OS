/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: boot                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Boot sequence of the OS. First Rust function called after       ║
   ║         assembly code: 'start'.                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, HHU                        ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use crate::consts;
use crate::device::pit::Timer;
use crate::device::ps2::{Keyboard, Mouse};
use crate::device::serial::SerialPort;
use crate::interrupt::interrupt_dispatcher;
use crate::memory::nvmem::Nfit;
use crate::memory::pages::page_table_index;
use crate::memory::vma::VmaType;
use crate::memory::{dram, nvmem, PAGE_SIZE};
use crate::process::thread::Thread;
use crate::syscall::{sys_vmem, syscall_dispatcher};
use crate::{
    acpi_tables, allocator, apic, gdt, get_initrd_frames,
    efi_services_available, init_acpi_tables, init_apic, init_boot_info,
    init_cpu_info, init_initrd, init_lfb, init_lfb_info, init_pci,
    init_serial_port, init_tty, initrd, keyboard, logger, mouse,
    process_manager, scheduler, serial_port, timer, tss,
};
use crate::{built_info, memory, naming, network, storage, infiniband};

/*
#[cfg(any(kernel_test, kernel_bench))]
use crate::{init_test_runner, run_tests}; */

#[cfg(any(kernel_test, kernel_bench))]
use crate::build_constants;
use crate::calibrate;
use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;
use chrono::DateTime;
use core::ffi::c_void;
use core::mem::size_of;
use core::ops::Deref;
use core::ptr;
use log::{trace, debug, info, warn, LevelFilter};
use multiboot2::{BootInformation, BootInformationHeader, EFIMemoryMapTag, MemoryAreaType, MemoryMapTag, TagHeader};
use uefi::data_types::Handle;
use uefi::mem::memory_map::MemoryMap;
use uefi::runtime::Time;
use uefi_raw::table::boot::MemoryType;
use uefi_raw::table::system::SystemTable;
use x86_64::PrivilegeLevel::Ring0;
use x86_64::instructions::interrupts;
use x86_64::instructions::segmentation::{CS, DS, ES, FS, GS, SS, Segment};
use x86_64::instructions::tables::load_tss;
use x86_64::registers::control::{Cr0, Cr0Flags, Cr3, Cr4, Cr4Flags};
use x86_64::registers::segmentation::SegmentSelector;
use x86_64::structures::gdt::Descriptor;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::{PageTable, PageTableFlags, PhysFrame};
use x86_64::{PhysAddr, VirtAddr};

// import labels from linker script 'link.ld'
unsafe extern "C" {
    static ___KERNEL_DATA_START__: c_void; // start address of OS image
    static ___KERNEL_DATA_END__: c_void; // end address of OS image
}

const BOOT_TO_GUI: bool = false; // Immediately start the GUI instead of terminal (Debug)

/// First Rust function called from assembly code `boot.asm` \
///   `multiboot2_magic` is the magic number read from 'eax' \
///   and `multiboot2_addr` is the address of multiboot2 info records
#[unsafe(no_mangle)]
pub extern "C" fn start(multiboot2_magic: u32, multiboot2_addr: *const BootInformationHeader) {
    // Initialize logger
    log::set_logger(logger())
        .map(|()| log::set_max_level(LevelFilter::Trace))
        .expect("Failed to initialize logger!");

    // Log messages and panics are now working, but cannot use format string until the heap is initialized later on
    info!("Welcome to D3OS early boot environment!");

    // Get multiboot information
    if multiboot2_magic != multiboot2::MAGIC {
        panic!("Invalid Multiboot2 magic number!");
    }

    // Search memory map, provided by bootloader or EFI, for usable memory and initialize physical memory management with free memory regions
    let multiboot = multiboot2_search_memory_map(multiboot2_addr);

    // Setup the GDT (Global Descriptor Table)
    // Has to be done after EFI boot services have been exited, since they rely on their own GDT
    info!("Initializing GDT");
    init_gdt();

    // The bootloader marks the kernel image region as available, so we need to reserve it manually
    let kernel_image_region = kernel_image_region();
    unsafe {
        memory::frames::boot_reserve(kernel_image_region);
    }
    // also reserve frames for initrd
    let initrd_tag = multiboot
        .module_tags()
        .find(|module| module.cmdline().is_ok_and(|name| name == "initrd"))
        .expect("Initrd not found!");
    let initrd_region = get_initrd_frames(initrd_tag);
    unsafe {
        memory::frames::boot_reserve(initrd_region);
    }
    // and the multiboot information
    let multiboot_region = get_multiboot_frames(&multiboot);
    unsafe {
        memory::frames::boot_reserve(multiboot_region);
    }

    // and initialize kernel heap, after which formatted strings may be used in logs and panics.
    info!("Initializing kernel heap");
    let heap_region = unsafe { memory::vmm::alloc_frames(consts::KERNEL_HEAP_PAGES) };
    unsafe {
        allocator().init(&heap_region);
    }
    info!("kernel image region: [Start: {:#x}, End: {:#x}]", 
        kernel_image_region.start.start_address().as_u64(), 
        kernel_image_region.end.start_address().as_u64(),
    );
    info!(
        "Initrd region: [Start: {:#x}, End: {:#x}]",
        initrd_region.start.start_address().as_u64(),
        initrd_region.end.start_address().as_u64(),
    );
    info!(
        "Multiboot region: [Start: {:#x}, End: {:#x}]",
        multiboot_region.start.start_address().as_u64(),
        multiboot_region.end.start_address().as_u64(),
    );
    trace!("{multiboot:?}");

    // Allocate frames for the kernel heap using the new way
    dram::alloc(consts::KERNEL_HEAP_PAGES as u64).expect("Failed to allocate kernel heap frames!");
    dram::dump();
    debug!("Old page frame allocator:\n{}", memory::frames::dump());
    #[cfg(any(kernel_test, kernel_bench))]
    {
        info!("Running D3OS in test mode!");
        info!("This Host => {} ({})\nTarget Host => {} ({})\n",
                build_constants::THIS_HOST, build_constants::THIS_IP,
                build_constants::TARGET_HOST, build_constants::TARGET_IP);
    }

    /*
        Hier den neuen Frame-Allocator aktivieren + Device Memory separat verwalten
     */

    // Merge reserved and free regions
    dram::finalize();
    dram::dump();
    debug!("Old page frame allocator:\n{}", memory::frames::dump());

    
    // Initialize CPU information
    init_cpu_info();

    // Create kernel process (and initialize virtual memory management)
    info!("Create kernel process and initialize paging");
    let kernel_process = process_manager().write().create_kernel_process(kernel_image_region, heap_region);
    kernel_process.virtual_address_space.page_tables().dump();
    kernel_process.virtual_address_space.load_address_space();

    // Initialize serial port and enable serial logging
    init_serial_port();
    if let Some(serial) = serial_port() {
        logger().register(serial);
    }

    // Map the framebuffer, needed for text output of the terminal
    let fb_info = multiboot
        .framebuffer_tag()
        .expect("No framebuffer information provided by bootloader!")
        .expect("Unknown framebuffer type!");
    let fb_start_phys_addr = fb_info.address();
    let fb_end_phys_addr = fb_start_phys_addr + (fb_info.height() * fb_info.pitch()) as u64;
    
    sys_vmem::init_fb_info(&fb_info);

    kernel_process.virtual_address_space.kernel_map_devm_identity(
        fb_start_phys_addr,
        fb_end_phys_addr,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE,
        VmaType::DeviceMemory,
        "framebuffer",
    );
    info!(
        "framebuffer region: [Start: {:#x}, End: {:#x}] ({}x{})",
        fb_start_phys_addr,
        fb_end_phys_addr,
        fb_info.width(),
        fb_info.height(),
        );

    // Initialize lfb info (For terminal_emulator)
    init_lfb_info(fb_info.address(), fb_info.pitch(), fb_info.width(), fb_info.height(), fb_info.bpp());
    // Initialize framebuffer (For window_manager)
    init_lfb(fb_info.address() as *mut u8, fb_info.pitch(), fb_info.width(), fb_info.height(), fb_info.bpp());

    // Dumping basic infos
    info!("Welcome to D3OS!");
    let version = format!(
        "v{} ({} - O{})",
        built_info::PKG_VERSION,
        built_info::PROFILE,
        built_info::OPT_LEVEL
    );
    let git_commit = built_info::GIT_COMMIT_HASH_SHORT.unwrap_or("Unknown");
    let build_date = match DateTime::parse_from_rfc2822(built_info::BUILT_TIME_UTC) {
        Ok(date_time) => date_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        Err(_) => "Unknown".to_string(),
    };
    let bootloader_name = match multiboot.boot_loader_name_tag() {
        Some(tag) => {
            if tag.name().is_ok() {
                tag.name().unwrap_or("Unknown")
            } else {
                "Unknown"
            }
        }
        None => "Unknown",
    };

    // Remember boot info
    init_boot_info(bootloader_name.to_string());

    info!("OS Version: [{}]", version);
    info!(
        "Git Version: [{} - {}]",
        built_info::GIT_HEAD_REF.unwrap_or_else(|| "Unknown"),
        git_commit
    );
    info!("Build Date: [{}]", build_date);
    info!("Compiler: [{}]", built_info::RUSTC_VERSION);
    info!("Bootloader: [{bootloader_name}]");

    // Initialize ACPI tables
    info!("Initializing ACPI tables");
    let rsdp_addr: usize = if let Some(rsdp_tag) = multiboot.rsdp_v2_tag() {
        ptr::from_ref(rsdp_tag) as usize + size_of::<TagHeader>()
    } else if let Some(rsdp_tag) = multiboot.rsdp_v1_tag() {
        ptr::from_ref(rsdp_tag) as usize + size_of::<TagHeader>()
    } else {
        panic!("ACPI not available!");
    };
    init_acpi_tables(rsdp_addr);

    interrupt_dispatcher::setup_idt();

    syscall_dispatcher::init();

    init_apic();

    // Initialize timer
    info!("Initializing timer");
    let timer = timer();
    Timer::plugin(Arc::clone(&timer));

    // Enable interrupts
    info!("Enabling interrupts");
    interrupts::enable();

    // Initialize EFI runtime service (if available and not done already during memory initialization)
    if uefi::table::system_table_raw().is_none() {
        match multiboot.efi_sdt64_tag() {
            Some(tag) => {
                info!("Initializing EFI runtime services");
                unsafe { uefi::table::set_system_table(tag.sdt_address() as *const SystemTable) };
            }
            None => warn!("Bootloader did not provide EFI system table pointer"),
        }
    }

    // Dump information about EFI runtime service
    info!(
        "EFI runtime services available (Vendor: [{}], UEFI version: [{}])",
        uefi::system::firmware_vendor(),
        uefi::system::uefi_revision()
    );

    // Initialize keyboard
    if let Some(keyboard) = keyboard() {
        Keyboard::plugin(keyboard);
    }

    if let Some(mouse) = mouse() {
        Mouse::plugin(mouse);
    }

    // Enable serial port interrupts
    if let Some(serial) = serial_port() {
        SerialPort::plugin(serial);
    }

    // Scan PCI bus
    info!("Scanning PCI bus");
    init_pci();

    // Initialize storage devices
    storage::init();

    // Initialize network stack
    network::init();

    /*#[cfg(any(kernel_test, kernel_bench))]
    init_test_runner(); */

    calibrate(50);
    infiniband::init();

    /*#[cfg(any(kernel_test, kernel_bench))]
    run_tests(); */

    // Initialize non-volatile memory (creates identity mappings for any non-volatile memory regions)
    nvmem::init();

    // As a demo for NVRAM support, we read the last boot time from NVRAM and write the current boot time to it
    if let Ok(nfit) = acpi_tables().lock().find_table::<Nfit>() {
        if let Some(range) = nfit.get_phys_addr_ranges().first() {
            let date_ptr = range.as_phys_frame_range().start.start_address().as_u64() as *mut Time;

            // Read last boot time from NVRAM
            let date = unsafe { date_ptr.read() };
            if date.is_valid().is_ok() {
                info!(
                    "Last boot time: [{:0>4}-{:0>2}-{:0>2} {:0>2}:{:0>2}:{:0>2}]",
                    date.year(),
                    date.month(),
                    date.day(),
                    date.hour(),
                    date.minute(),
                    date.second()
                );
            }

            // Write current boot time to NVRAM
            if efi_services_available() {
                if let Ok(time) = uefi::runtime::get_time() {
                    unsafe { date_ptr.write(time) }
                }
            }
        }
    }

    // Load initial ramdisk
    init_initrd(initrd_tag);

    // Init naming service
    naming::api::init();

    // Create and register the cleanup thread in the scheduler
    // (If the last thread of a process terminates, it cannot delete its own address space)
    extern "sysv64" fn cleanup() {
        loop {
            scheduler().sleep(100);
            process_manager().write().drop_exited_process();
        }
    }
    scheduler().ready(Thread::new_kernel_thread(cleanup, "cleanup"));

    //Initialize tty buffer (Workaround for missing pipes)
    init_tty();

    if BOOT_TO_GUI {
        // Create and register the 'window_manager' thread in the scheduler
        scheduler().ready(Thread::load_application(initrd().entries()
            .find(|entry| entry.filename().as_str().unwrap() == "bin/window_manager")
            .expect("Window Manager application not available!")
            .data(), "window_manager", &[].to_vec()));
    } else {
        // Create and register the 'terminal_emulator' thread (from app image in ramdisk) in the scheduler
        scheduler().ready(Thread::load_application(
            initrd()
                .entries()
                .find(|entry| entry.filename().as_str().unwrap() == "bin/terminal_emulator")
                .expect("Terminal application not available!")
                .data(),
            "terminal_emulator",
            &[].to_vec(),
        ));
    }

    // Dump information about all processes (including VMAs)
    process_manager().read().dump();

    // Start APIC timer & scheduler
    info!("Starting scheduler");
    apic().start_timer(10);

    scheduler().start();
}

/// Set up the GDT
fn init_gdt() {
    let mut gdt = gdt().lock();
    let tss = tss().lock();

    gdt.append(Descriptor::kernel_code_segment());
    gdt.append(Descriptor::kernel_data_segment());
    gdt.append(Descriptor::user_data_segment());
    gdt.append(Descriptor::user_code_segment());

    unsafe {
        // We need to obtain a static reference to the TSS and GDT for the following operations.
        // We know, that they have a static lifetime, since they are declared as static variables in 'kernel/mod.rs'.
        // However, since they are hidden behind a Mutex, the borrow checker does not see them with a static lifetime.
        let gdt_ref = ptr::from_ref(gdt.deref()).as_ref().unwrap();
        let tss_ref = ptr::from_ref(tss.deref()).as_ref().unwrap();
        gdt.append(Descriptor::tss_segment(tss_ref));
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

/// Return `PhysFrameRange` for memory occupied by the kernel image
fn kernel_image_region() -> PhysFrameRange {
    let start: PhysFrame;
    let end: PhysFrame;

    unsafe {
        start = PhysFrame::from_start_address(PhysAddr::new(ptr::from_ref(&___KERNEL_DATA_START__) as u64)).expect("Kernel code is not page aligned");
        end = PhysFrame::from_start_address(PhysAddr::new(ptr::from_ref(&___KERNEL_DATA_END__) as u64).align_up(PAGE_SIZE as u64)).unwrap();
    }

    PhysFrameRange { start, end }
}

/// Return `PhysFrameRange` for memory occupied by the multiboot info struct.
fn get_multiboot_frames(multiboot: &BootInformation<'_>) -> PhysFrameRange {
    PhysFrameRange {
        start: PhysFrame::containing_address(PhysAddr::new(multiboot.start_address() as u64)),
        end: PhysFrame::containing_address(PhysAddr::new(multiboot.end_address() as u64)),
    }
}

/// Identifies usable memory and initialize physical memory management \
/// and returns `BootInformation` by searching the memory maps, provided by bootloader of EFI. \
///   `multiboot2_addr` is the address of multiboot2 info records
fn multiboot2_search_memory_map(multiboot2_addr: *const BootInformationHeader) -> BootInformation<'static> {
    let multiboot = unsafe { BootInformation::load(multiboot2_addr).expect("Failed to get Multiboot2 information") };

    // Search memory map, provided by bootloader of EFI, for usable memory and initialize physical memory management
    if multiboot.efi_bs_not_exited_tag().is_some() {
        // EFI boot services have not been exited, and we obtain access to the memory map and EFI runtime services by exiting them manually
        info!("EFI boot services have not been exited yet");
        let image_tag = multiboot.efi_ih64_tag().expect("EFI image handle not available!");
        let sdt_tag = multiboot.efi_sdt64_tag().expect("EFI system table not available!");
        let memory_map;

        unsafe {
            let image_handle = Handle::from_ptr(image_tag.image_handle() as *mut c_void).expect("Failed to create EFI image handle struct from pointer!");
            uefi::table::set_system_table(sdt_tag.sdt_address() as *const SystemTable);
            uefi::boot::set_image_handle(image_handle);

            info!("Exiting EFI boot services to obtain runtime system table and memory map");
            memory_map = uefi::boot::exit_boot_services(MemoryType::LOADER_DATA);
        }

        scan_efi_memory_map(&memory_map);
    } else {
        info!("EFI boot services have already been exited by the bootloader");
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
    multiboot
}

/// Searching available memory regions provided by multiboot2 in `memory map`. \
/// Available only if efi boot services have been exited and bootloader provides these memory maps.
fn scan_multiboot2_memory_map(memory_map: &MemoryMapTag) {
    info!("Searching memory map for available regions");
    memory_map
        .memory_areas()
        .iter()
        .filter(|area| area.typ() == MemoryAreaType::Available)
        .for_each(|area| unsafe {
            memory::frames::boot_avail(PhysFrameRange {
                start: PhysFrame::from_start_address(PhysAddr::new(area.start_address()).align_up(PAGE_SIZE as u64)).unwrap(),
                end: PhysFrame::from_start_address(PhysAddr::new(area.end_address()).align_down(PAGE_SIZE as u64)).unwrap(),
            });
        });
}

/// Memory map from efi. Only available if boot services have been exited. \
/// Sometimes bootloaders do not provide multiboot2 memory maps if \
/// efi information has been requested.
fn scan_efi_multiboot2_memory_map(memory_map: &EFIMemoryMapTag) {
    info!("Searching memory map for available regions");
    memory_map
        .memory_areas()
        .filter(|area| {
            area.ty.0 == MemoryType::CONVENTIONAL.0
                || area.ty.0 == MemoryType::LOADER_CODE.0
                || area.ty.0 == MemoryType::LOADER_DATA.0
                || area.ty.0 == MemoryType::BOOT_SERVICES_CODE.0
                || area.ty.0 == MemoryType::BOOT_SERVICES_DATA.0
        }) // .0 necessary because of different version dependencies to uefi-crate
        .for_each(|area| {
            let start = PhysFrame::from_start_address(PhysAddr::new(area.phys_start).align_up(PAGE_SIZE as u64)).unwrap();
            let frames = PhysFrame::range(start, start + area.page_count);

            // Non-conventional memory may be write-protected, and we need to unprotect it first
            if area.ty.0 != MemoryType::CONVENTIONAL.0 {
                unprotect_frames(frames);
            }

            unsafe {
                memory::frames::boot_avail(frames);
            }
        });
}

/// Memory map from efi. Only available if boot services have NOT been exited.
fn scan_efi_memory_map(memory_map: &dyn MemoryMap) {
    info!("Searching memory map for available regions");
    memory_map
        .entries()
        .filter(|area| {
            area.ty == MemoryType::CONVENTIONAL
                || area.ty == MemoryType::LOADER_CODE
                || area.ty == MemoryType::LOADER_DATA
                || area.ty == MemoryType::BOOT_SERVICES_CODE
                || area.ty == MemoryType::BOOT_SERVICES_DATA
        })
        .for_each(|area| {
            let start = PhysFrame::from_start_address(PhysAddr::new(area.phys_start).align_up(PAGE_SIZE as u64)).unwrap();
            let frames = PhysFrame::range(start, start + area.page_count);

            // Non-conventional memory may be write-protected, and we need to unprotect it first
            if area.ty != MemoryType::CONVENTIONAL {
                unprotect_frames(frames);
            }

            unsafe {
                memory::frames::boot_avail(frames);
            }
        });
}

fn unprotect_frames(frames: PhysFrameRange) {
    unsafe { Cr0::update(|flags| flags.remove(Cr0Flags::WRITE_PROTECT)) };

    let root_level = if Cr4::read().contains(Cr4Flags::L5_PAGING) { 5 } else { 4 };
    for frame in frames {
        unprotect_frame(frame, root_level);
    }

    unsafe { Cr0::update(|flags| flags.insert(Cr0Flags::WRITE_PROTECT)) };
}

fn unprotect_frame(frame: PhysFrame, root_level: usize) {
    let addr = VirtAddr::new(frame.start_address().as_u64());
    let mut page_table = unsafe { (Cr3::read().0.start_address().as_u64() as *mut PageTable).as_mut().unwrap() };

    let mut level = root_level;
    loop {
        let index = page_table_index(addr, level);
        let entry = &mut page_table[index];
        let flags = entry.flags();

        if level == 1 || flags.contains(PageTableFlags::HUGE_PAGE) {
            entry.set_flags(flags | PageTableFlags::WRITABLE);
            break;
        }

        page_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
        level -= 1;
    }
}
