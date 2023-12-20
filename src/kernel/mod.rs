use spin::Mutex;
use uefi::table::{Runtime, SystemTable};
use x86_64::structures::gdt::GlobalDescriptorTable;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use crate::kernel::log::Logger;
use crate::kernel::service::device_service::DeviceService;
use crate::kernel::service::interrupt_service::InterruptService;
use crate::kernel::service::memory_service::MemoryService;
use crate::kernel::service::thread_service::ThreadService;
use crate::kernel::service::time_service::TimeService;

pub mod service;
pub mod log;
pub mod thread;
pub mod interrupt;
pub mod syscall;

#[global_allocator]
static mut MEMORY_SERVICE: MemoryService = MemoryService::new();
static mut INTERRUPT_SERVICE: InterruptService = InterruptService::new();
static mut DEVICE_SERVICE: DeviceService = DeviceService::new();
static mut THREAD_SERVICE: ThreadService = ThreadService::new();
static mut TIME_SERVICE: TimeService = TimeService::new();
static mut EFI_SYSTEM_TABLE: Option<SystemTable<Runtime>> = None;

static LOGGER: Mutex<Logger> = Mutex::new(Logger::new());

static GDT: Mutex<GlobalDescriptorTable> = Mutex::new(GlobalDescriptorTable::new());
static TSS: Mutex<TaskStateSegment> = Mutex::new(TaskStateSegment::new());

pub trait Service {}

pub fn get_memory_service() -> &'static mut MemoryService {
    unsafe { return &mut MEMORY_SERVICE }
}

pub fn get_interrupt_service() -> &'static mut InterruptService {
    unsafe { return &mut INTERRUPT_SERVICE }
}

pub fn get_device_service() -> &'static mut DeviceService {
    unsafe { return &mut DEVICE_SERVICE }
}

pub fn get_thread_service() -> &'static mut ThreadService {
    unsafe { return &mut THREAD_SERVICE }
}

pub fn get_time_service() -> &'static mut TimeService {
    unsafe { return &mut TIME_SERVICE }
}

pub fn get_efi_system_table() -> &'static Option<SystemTable<Runtime>> {
    unsafe { return &mut EFI_SYSTEM_TABLE; }
}

pub fn set_efi_system_table(table: SystemTable<Runtime>) {
    unsafe { EFI_SYSTEM_TABLE = Some(table); }
}

pub fn get_logger() -> &'static Mutex<Logger> {
    return &LOGGER;
}

pub fn get_gdt() -> &'static Mutex<GlobalDescriptorTable> {
    return &GDT;
}

pub fn get_tss() -> &'static Mutex<TaskStateSegment> {
    return &TSS;
}

#[no_mangle]
pub extern "C" fn tss_set_rsp0(rsp0: u64) {
    get_tss().lock().privilege_stack_table[0] = VirtAddr::new(rsp0);
}