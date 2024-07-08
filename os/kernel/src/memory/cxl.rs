use alloc::vec::Vec;
use crate::device::pci::PciBus;
use crate::pci_bus;

pub fn print_bus_devices(){
    pci_bus().dump_devices();
}