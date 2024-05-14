#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(const_mut_refs)]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(exact_size_is_empty)]
#![feature(panic_info_message)]
#![feature(fmt_internals)]
#![feature(abi_x86_interrupt)]
#![feature(trait_upcasting)]
#![allow(internal_features)]
#![no_std]

use crate::device::apic::Apic;
use crate::device::lfb_terminal::{CursorThread, LFBTerminal};
use crate::device::pit::Timer;
use crate::device::ps2::PS2;
use crate::device::serial;
use crate::device::serial::{BaudRate, ComPort, SerialPort};
use crate::device::speaker::Speaker;
use crate::device::terminal::Terminal;
use crate::memory::alloc::{AcpiHandler, KernelAllocator};
use crate::interrupt::interrupt_dispatcher::InterruptDispatcher;
use crate::log::Logger;
use crate::process::scheduler::Scheduler;
use crate::process::thread::Thread;
use alloc::boxed::Box;
use core::fmt::Arguments;
use core::panic::PanicInfo;
use ::log::{error, Level, Log, Record};
use acpi::AcpiTables;
use multiboot2::ModuleTag;
use spin::{Mutex, Once, RwLock};
use tar_no_std::TarArchiveRef;
use uefi::table::{Runtime, SystemTable};
use x86_64::structures::gdt::GlobalDescriptorTable;
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::PhysFrame;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::{PhysAddr, VirtAddr};
use crate::device::pci::PciBus;
use crate::memory::PAGE_SIZE;
use crate::process::process::ProcessManager;

extern crate alloc;

#[macro_use]
pub mod device;
pub mod boot;
pub mod interrupt;
pub mod memory;
pub mod log;
pub mod syscall;
pub mod process;

pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if terminal_initialized() {
        println!("Panic: {}", info);
    } else {
        let record = Record::builder()
            .level(Level::Error)
            .file(Some("panic"))
            .args(*info.message().unwrap_or(&Arguments::new_const(&["A panic occurred!"])))
            .build();

        unsafe { logger().force_unlock() };
        let log = logger().lock();
        unsafe { logger().force_unlock() }; // log() also calls logger().lock()
        log.log(&record);
    }

    loop {}
}

struct EfiSystemTable {
    table: RwLock<SystemTable<Runtime>>,
}

unsafe impl Send for EfiSystemTable {}
unsafe impl Sync for EfiSystemTable {}

impl EfiSystemTable {
    const fn new(table: SystemTable<Runtime>) -> Self {
        Self { table: RwLock::new(table) }
    }
}

static GDT: Mutex<GlobalDescriptorTable> = Mutex::new(GlobalDescriptorTable::new());
static TSS: Mutex<TaskStateSegment> = Mutex::new(TaskStateSegment::new());
static IDT: Mutex<InterruptDescriptorTable> = Mutex::new(InterruptDescriptorTable::new());
static EFI_SYSTEM_TABLE: Once<EfiSystemTable> = Once::new();
static ACPI_TABLES: Once<Mutex<AcpiTables<AcpiHandler>>> = Once::new();
static INIT_RAMDISK: Once<TarArchiveRef> = Once::new();

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator::new();
static LOGGER: Mutex<Logger> = Mutex::new(Logger::new());
static PROCESS_MANAGER: RwLock<ProcessManager> = RwLock::new(ProcessManager::new());
static SCHEDULER: Once<Scheduler> = Once::new();
static INTERRUPT_DISPATCHER: Once<InterruptDispatcher> = Once::new();

static APIC: Once<Apic> = Once::new();
static TIMER: RwLock<Timer> = RwLock::new(Timer::new());
static SPEAKER: Mutex<Speaker> = Mutex::new(Speaker::new());
static SERIAL_PORT: Once<SerialPort> = Once::new();
static TERMINAL: Once<LFBTerminal> = Once::new();
static PS2: Once<PS2> = Once::new();
static PCI: Once<PciBus> = Once::new();

pub fn init_efi_system_table(table: SystemTable<Runtime>) {
    EFI_SYSTEM_TABLE.call_once(|| EfiSystemTable::new(table));
}

pub fn init_acpi_tables(rsdp_addr: usize) {
    ACPI_TABLES.call_once(|| {
        let handler = AcpiHandler::default();

        unsafe {
            let tables = AcpiTables::from_rsdp(handler, rsdp_addr);
            match tables {
                Ok(tables) => Mutex::new(tables),
                Err(_) => panic!("Failed to parse ACPI tables"),
            }
        }
    });
}

pub fn init_apic() {
    APIC.call_once(|| Apic::new());
}

pub fn init_serial_port() {
    let mut serial: Option<SerialPort> = None;
    if serial::check_port(ComPort::Com1) {
        serial = Some(SerialPort::new(ComPort::Com1));
    } else if serial::check_port(ComPort::Com2) {
        serial = Some(SerialPort::new(ComPort::Com2));
    } else if serial::check_port(ComPort::Com3) {
        serial = Some(SerialPort::new(ComPort::Com3));
    } else if serial::check_port(ComPort::Com4) {
        serial = Some(SerialPort::new(ComPort::Com4));
    }

    if serial.is_some() {
        serial.as_mut().unwrap().init(128, BaudRate::Baud115200);
        SERIAL_PORT.call_once(|| serial.unwrap());
    }
}

pub fn init_terminal(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) {
    let terminal = LFBTerminal::new(buffer, pitch, width, height, bpp);
    terminal.clear();
    TERMINAL.call_once(|| terminal);

    scheduler().ready(Thread::new_kernel_thread(Box::new(|| {
        let mut cursor_thread = CursorThread::new(&TERMINAL.get().unwrap());
        cursor_thread.run();
    })));
}

pub fn init_keyboard() {
    PS2.call_once(|| {
        let mut ps2 = PS2::new();
        match ps2.init_controller() {
            Ok(_) => {
                match ps2.init_keyboard() {
                    Ok(_) => {}
                    Err(error) => error!("Keyboard initialization failed: {:?}", error)
                }
            }
            Err(error) => error!("PS/2 controller initialization failed: {:?}", error)
        }

        return ps2;
    });
}

pub fn init_pci() {
    PCI.call_once(|| PciBus::scan());
}

pub fn init_initrd(module: &ModuleTag) {
    INIT_RAMDISK.call_once(|| {
        let initrd_frames = PhysFrameRange {
            start: PhysFrame::from_start_address(PhysAddr::new(module.start_address() as u64)).expect("Initial ramdisk is not page aligned"),
            end: PhysFrame::from_start_address(PhysAddr::new(module.end_address() as u64).align_up(PAGE_SIZE as u64)).unwrap(),
        };
        unsafe { memory::physical::reserve(initrd_frames); }

        let initrd_bytes = unsafe { core::slice::from_raw_parts(module.start_address() as *const u8, (module.end_address() - module.start_address()) as usize) };
        TarArchiveRef::new(initrd_bytes).expect("Failed to create TarArchiveRef from Multiboot2 module")
    });
}

pub fn terminal_initialized() -> bool {
    TERMINAL.get().is_some()
}

pub fn gdt() -> &'static Mutex<GlobalDescriptorTable> {
    &GDT
}

pub fn tss() -> &'static Mutex<TaskStateSegment> {
    &TSS
}

pub fn idt() -> &'static Mutex<InterruptDescriptorTable> {
    &IDT
}

pub fn acpi_tables() -> &'static Mutex<AcpiTables<AcpiHandler>> {
    ACPI_TABLES.get().expect("Trying to access ACPI tables before initialization!")
}

pub fn efi_system_table() -> Option<&'static RwLock<SystemTable<Runtime>>> {
    match EFI_SYSTEM_TABLE.get() {
        Some(wrapper) => Some(&wrapper.table),
        None => None,
    }
}

pub fn initrd() -> &'static TarArchiveRef<'static> {
    &INIT_RAMDISK.get().expect("Trying to access initial ramdisk before initialization!")
}

pub fn allocator() -> &'static KernelAllocator {
    &ALLOCATOR
}

pub fn logger() -> &'static Mutex<Logger> {
    &LOGGER
}

pub fn interrupt_dispatcher() -> &'static InterruptDispatcher {
    INTERRUPT_DISPATCHER.call_once(|| InterruptDispatcher::new());
    INTERRUPT_DISPATCHER.get().unwrap()
}

pub fn process_manager() -> &'static RwLock<ProcessManager> {
    &PROCESS_MANAGER
}

pub fn scheduler() -> &'static Scheduler {
    SCHEDULER.call_once(|| Scheduler::new());
    &SCHEDULER.get().unwrap()
}

pub fn apic() -> &'static Apic {
    APIC.get().expect("Trying to access APIC before initialization!")
}

pub fn timer() -> &'static RwLock<Timer> {
    &TIMER
}

pub fn speaker() -> &'static Mutex<Speaker> {
    &SPEAKER
}

pub fn serial_port() -> Option<&'static SerialPort> {
    SERIAL_PORT.get()
}

pub fn terminal() -> &'static dyn Terminal {
    TERMINAL.get().expect("Trying to access terminal before initialization!")
}

pub fn ps2_devices() -> &'static PS2 {
    PS2.get().expect("Trying to access PS/2 devices before initialization!")
}

pub fn pci_bus() -> &'static PciBus {
    PCI.get().expect("Trying to access PCI bus before initialization!")
}

#[no_mangle]
pub extern "C" fn tss_set_rsp0(rsp0: u64) {
    tss().lock().privilege_stack_table[0] = VirtAddr::new(rsp0);
}

#[no_mangle]
pub extern "C" fn tss_get_rsp0() -> u64 {
    tss().lock().privilege_stack_table[0].as_u64()
}
