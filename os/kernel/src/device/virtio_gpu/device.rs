use log::info;
use crate::pci_bus;
use super::config::*;
use super::queue::*;
use super::command::*;

pub struct VirtioGpuDevice {
    // z.B. PCI Base Address, Virtqueues, Config, ...
}

impl VirtioGpuDevice {
    pub fn new(/* Parameter */) -> Self {
        // Instanziere das Device
        Self { /* Felder initialisieren */ }
    }

    pub fn draw_line(&mut self, x0: u32, y0: u32, x1: u32, y1: u32) {
        // Erzeuge Virtio-GPU-Kommandos und reiche sie an die Queue weiter
    }
}

pub fn init_device() {
    let devices = pci_bus().search_by_ids(0x1AF4, 0x1050);
    for device in devices {
        let (vendor_id, device_id) = device.read().header().id(&pci_bus().config_space());
        info!("Found GPU [{}:{}]", vendor_id, device_id);
    }
}