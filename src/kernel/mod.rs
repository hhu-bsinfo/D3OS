use crate::kernel::service::device_service::DeviceService;
use crate::kernel::service::interrupt_service::InterruptService;
use crate::kernel::service::log_service::LogService;
use crate::kernel::service::memory_service::MemoryService;
use crate::kernel::service::thread_service::ThreadService;

pub mod interrupt_dispatcher;
pub mod isr;
pub mod service;
pub mod log;
pub mod thread;

#[global_allocator]
static mut MEMORY_SERVICE: MemoryService = MemoryService::new();
static mut INTERRUPT_SERVICE: InterruptService = InterruptService::new();
static mut DEVICE_SERVICE: DeviceService = DeviceService::new();
static mut LOG_SERVICE: LogService = LogService::new();
static mut THREAD_SERVICE: ThreadService = ThreadService::new();

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

pub fn get_log_service() -> &'static mut LogService {
    unsafe { return &mut LOG_SERVICE }
}

pub fn get_thread_service() -> &'static mut ThreadService {
    unsafe { return &mut THREAD_SERVICE }
}