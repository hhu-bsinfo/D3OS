use ::alloc::boxed::Box;
use acpi::AcpiTables;
use spin::{Mutex, Once, RwLock};
use uefi::table::{Runtime, SystemTable};
use x86_64::structures::gdt::GlobalDescriptorTable;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use crate::device::apic::Apic;
use crate::device::lfb_terminal::{CursorThread, LFBTerminal};
use crate::device::speaker::Speaker;
use crate::device::pit::Timer;
use crate::device::ps2::PS2;
use crate::device::serial;
use crate::device::serial::{BaudRate, ComPort, SerialPort};
use crate::device::terminal::Terminal;
use crate::kernel::alloc::{AcpiHandler, KernelAllocator};
use crate::kernel::interrupt::interrupt_dispatcher::InterruptDispatcher;
use crate::kernel::log::Logger;
use crate::kernel::thread::scheduler::Scheduler;
use crate::kernel::thread::thread::Thread;

pub mod alloc;
pub mod log;
pub mod thread;
pub mod interrupt;
pub mod syscall;

struct EfiSystemTable {
    table: SystemTable<Runtime>
}

unsafe impl Send for EfiSystemTable {}
unsafe impl Sync for EfiSystemTable {}

impl EfiSystemTable {
    const fn new(table: SystemTable<Runtime>) -> Self {
        Self { table }
    }
}

static GDT: Mutex<GlobalDescriptorTable> = Mutex::new(GlobalDescriptorTable::new());
static TSS: Mutex<TaskStateSegment> = Mutex::new(TaskStateSegment::new());
static EFI_SYSTEM_TABLE: Once<EfiSystemTable> = Once::new();
static ACPI_TABLES: Once<Mutex<AcpiTables<AcpiHandler>>> = Once::new();

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator::new();
static LOGGER: Mutex<Logger> = Mutex::new(Logger::new());
static SCHEDULER: Once<Scheduler> = Once::new();
static INTERRUPT_DISPATCHER: Once<InterruptDispatcher> = Once::new();

static APIC: Once<Apic> = Once::new();
static TIMER: RwLock<Timer> = RwLock::new(Timer::new());
static SPEAKER: Mutex<Speaker> = Mutex::new(Speaker::new());
static SERIAL_PORT: Once<SerialPort> = Once::new();
static TERMINAL: Once<LFBTerminal> = Once::new();
static PS2: Once<PS2> = Once::new();

pub trait Service {}

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
                Err(_) => panic!("Failed to parse ACPI tables")
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
    TERMINAL.call_once(|| LFBTerminal::new(buffer, pitch, width, height, bpp));

    scheduler().ready(Thread::new_kernel_thread(Box::new(|| {
        let mut cursor_thread = CursorThread::new(&TERMINAL.get().unwrap());
        cursor_thread.run();
    })))
}

pub fn init_keyboard() {
    PS2.call_once(|| {
        let mut ps2 = PS2::new();
        ps2.init_controller().unwrap_or_else(|err| panic!("Failed to initialize PS2 controller (Error: {:?})", err));
        ps2.init_keyboard().unwrap_or_else(|err| panic!("Failed to initialize PS2 keyboard (Error: {:?})", err));

        return ps2;
    });
}

pub fn terminal_initialized() -> bool {
    return TERMINAL.get().is_some();
}

pub fn gdt() -> &'static Mutex<GlobalDescriptorTable> {
    return &GDT;
}

pub fn tss() -> &'static Mutex<TaskStateSegment> {
    return &TSS;
}

pub fn acpi_tables() -> &'static Mutex<AcpiTables<AcpiHandler>> {
    return ACPI_TABLES.get().expect("Trying to access ACPI tables before initialization!");
}

pub fn efi_system_table() -> Option<&'static SystemTable<Runtime>> {
    return match EFI_SYSTEM_TABLE.get() {
        Some(wrapper) => Some(&wrapper.table),
        None => None
    }
}

pub fn allocator() -> &'static KernelAllocator {
    return &ALLOCATOR;
}

pub fn logger() -> &'static Mutex<Logger> {
    return &LOGGER;
}

pub fn interrupt_dispatcher() -> &'static InterruptDispatcher {
    INTERRUPT_DISPATCHER.call_once(|| InterruptDispatcher::new());
    return INTERRUPT_DISPATCHER.get().unwrap();
}

pub fn scheduler() -> &'static Scheduler {
    SCHEDULER.call_once(|| Scheduler::new());
    return &SCHEDULER.get().unwrap();
}

pub fn apic() -> &'static Apic {
    return APIC.get().expect("Trying to access APIC before initialization!");
}

pub fn timer() -> &'static RwLock<Timer> {
    return &TIMER;
}

pub fn speaker() -> &'static Mutex<Speaker> {
    return &SPEAKER;
}

pub fn serial_port() -> Option<&'static SerialPort> {
    return SERIAL_PORT.get();
}

pub fn terminal() -> &'static dyn Terminal {
    return TERMINAL.get().expect("Trying to access terminal before initialization!");
}

pub fn ps2_devices() -> &'static PS2 {
    return PS2.get().expect("Trying to access keyboard before initialization!");
}

#[no_mangle]
pub extern "C" fn tss_set_rsp0(rsp0: u64) {
    tss().lock().privilege_stack_table[0] = VirtAddr::new(rsp0);
}

#[no_mangle]
pub extern "C" fn tss_get_rsp0() -> u64 {
    return tss().lock().privilege_stack_table[0].as_u64();
}

#[no_mangle]
pub extern "C" fn dispatch_interrupt(int_number: u32) {
    interrupt_dispatcher().dispatch(int_number);
}