pub mod config;
pub mod device;
pub mod queue;
pub mod command;

pub use device::VirtioGpuDevice;

pub fn init() {
    device::init_device();
}
