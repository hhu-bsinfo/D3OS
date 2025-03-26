use alloc::sync::Arc;
use log::info;
use crate::pci_bus;
use spin::{Once};
use crate::device::virtio::gpu::gpu::VirtioGpu;

pub mod gpu;

pub const VIRTIO_GPU_F_VERSION_1: u32 = 1 << 0;
pub const VIRTIO_GPU_PCI_VENDOR_ID: u16 = 0x1AF4;
pub const VIRTIO_GPU_PCI_DEVICE_ID: u16 = 0x1050;

static VIRTIOGPU: Once<Arc<VirtioGpu>> = Once::new();



pub fn init() {
    let devices = pci_bus().search_by_ids(VIRTIO_GPU_PCI_VENDOR_ID, VIRTIO_GPU_PCI_DEVICE_ID);
    if devices.len() > 0 {
        let (vendor_id, device_id) = devices[0].read().header().id(&pci_bus().config_space());
        VIRTIOGPU.call_once(|| {
            info!("LEngth of devices: {}", devices.len());
            info!("Found Virtio GPU device: {:X}:{:X}", vendor_id, device_id);
            let gpu = Arc::new(VirtioGpu::new(devices[0]));
            gpu
        });
    }


}