use crate::kernel::service::device_service::DeviceService;
use crate::kernel::service::interrupt_service::InterruptService;
use crate::kernel::service::memory_service::MemoryService;

pub mod interrupt_dispatcher;
pub mod isr;
pub mod service;

#[global_allocator]
static mut MEMORY_SERVICE: MemoryService = MemoryService::new();
static mut INTERRUPT_SERVICE: InterruptService = InterruptService::new();
static mut DEVICE_SERIVCE: DeviceService = DeviceService::new();

pub trait Service {}

pub fn get_memory_service() -> &'static mut MemoryService {
    unsafe { return &mut MEMORY_SERVICE }
}

pub fn get_interrupt_service() -> &'static mut InterruptService {
    unsafe { return &mut INTERRUPT_SERVICE }
}

pub fn get_device_service() -> &'static mut DeviceService {
    unsafe { return &mut DEVICE_SERIVCE }
}