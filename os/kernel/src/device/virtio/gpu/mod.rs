use log::info;
use crate::device::virtio::gpu::device::{VIRTIO_GPU_PCI_DEVICE_ID, VIRTIO_GPU_PCI_VENDOR_ID};
use crate::pci_bus;

pub mod device;

pub fn init() {
    let devices = pci_bus().search_by_ids(VIRTIO_GPU_PCI_VENDOR_ID, VIRTIO_GPU_PCI_DEVICE_ID);
    for device in devices {
        let (vendor_id, device_id) = device.read().header().id(&pci_bus().config_space());
        info!("Found Virtio GPU device: {:X}:{:X}", vendor_id, device_id);
        device.call_once(|device| {
            info!("Virtio GPU device at [{:?}]", device.address());
        });


        //let gpu = Arc::new(VirtioGpuDevice::new(device));
        //gpu.lock().init();
    }
}