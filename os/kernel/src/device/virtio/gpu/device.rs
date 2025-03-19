use alloc::sync::Arc;
use crate::pci_bus;
use crate::device::virtio::virtqueue::*;
use crate::device::virtio::command::*;
use log::info;
use pci_types::EndpointHeader;
use spin::Mutex;
use x86_64::instructions::port::Port;

pub const VIRTIO_GPU_F_VERSION_1: u32 = 1 << 0;
pub const VIRTIO_GPU_PCI_VENDOR_ID: u16 = 0x1AF4;
pub const VIRTIO_GPU_PCI_DEVICE_ID: u16 = 0x1050;

pub struct VirtioGpuDevice {
    device: Arc<Mutex<EndpointHeader>>,
    io_base: u16,
    irq: u8,
}

impl VirtioGpuDevice {
    pub fn new(device: Arc<Mutex<EndpointHeader>>) -> Self {
        //let io_base = device.lock().bar(0, ()) as u16;
        //let irq = device.lock().interrupt_line();
        Self { device, io_base, irq }
    }

    pub fn init(&self) {
        info!("Initializing Virtio GPU driver...");
        self.reset_device();
        self.setup_features();
        self.setup_queues();
        self.enable_interrupts();
    }

    fn reset_device(&self) {
        let mut status_port = Port::<u32>::new(self.io_base + 0x14);
        unsafe { status_port.write(0); }
    }

    fn setup_features(&self) {
        let mut features_port = Port::<u32>::new(self.io_base + 0x10);
        let features = unsafe { features_port.read() };
        info!("Device features: {:#X}", features);
    }

    fn setup_queues(&self) {
        info!("Setting up Virtqueues...");
    }

    fn enable_interrupts(&self) {
        info!("Enabling interrupts for Virtio GPU...");
    }
}

// device/virtio_gpu/init.rs
pub fn init() {
    let devices = pci_bus().search_by_ids(VIRTIO_GPU_PCI_VENDOR_ID, VIRTIO_GPU_PCI_DEVICE_ID);
    for device in devices {
        let (vendor_id, device_id) = device.read().header().id(&pci_bus().config_space());
        info!("Found Virtio GPU device: {:X}:{:X}", vendor_id, device_id);


        //let gpu = Arc::new(VirtioGpuDevice::new(device));
        //gpu.lock().init();
    }
}

/*
// GPU Commands
#[repr(C)]
pub struct VirtioGpuCtrlHdr {
    // Command header fields
}

#[repr(C)]
pub struct VirtioGpuResourceCreate2d {
    // Fields for resource creation
}

#[repr(C)]
pub struct VirtioGpuResourceFlush {
    // Fields for resource flush
}*/