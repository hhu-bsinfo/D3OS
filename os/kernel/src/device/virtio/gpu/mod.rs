use log::info;
use crate::pci_bus;
use pci_types::{ConfigRegionAccess, EndpointHeader, PciAddress};
use pci_types::capability::PciCapability;
use spin::RwLock;

pub mod gpu;

pub const VIRTIO_GPU_F_VERSION_1: u32 = 1 << 0;
pub const VIRTIO_GPU_PCI_VENDOR_ID: u16 = 0x1AF4;
pub const VIRTIO_GPU_PCI_DEVICE_ID: u16 = 0x1050;



pub fn init() {
    let devices = pci_bus().search_by_ids(VIRTIO_GPU_PCI_VENDOR_ID, VIRTIO_GPU_PCI_DEVICE_ID);
    for device in devices {
        let (vendor_id, device_id) = device.read().header().id(&pci_bus().config_space());
        info!("Found Virtio GPU device: {:X}:{:X}", vendor_id, device_id);
        //let gpu = Arc::new(VirtioGpuDevice::new(device));
        //gpu.lock().init();
    }
}