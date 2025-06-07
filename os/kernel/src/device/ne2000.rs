use crate::{apic, interrupt_dispatcher, pci_bus, process_manager, scheduler};
use bitflags::bitflags;
use log::info;
use pci_types::{CommandRegister, EndpointHeader};
use spin::{Mutex, RwLock};
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

bitflags! {
    pub struct PageRegisters :u16 {
        const COMMAND = 0x00; // R/W Command for Pages 0, 1, 2
        const IOPORT = 0x10;

    }
}

bitflags! {
    pub struct InterruptFlags :u8 {
        const ISR_RST = 0x80;
    }
}

bitflags! {
    pub struct Command :u8 {
        const RESET = 0x1F;
    }
}

pub struct Registers {
    id: Mutex<(
        PortReadOnly<u8>,
        PortReadOnly<u8>,
        PortReadOnly<u8>,
        PortReadOnly<u8>,
        PortReadOnly<u8>,
        PortReadOnly<u8>,
    )>,
    command: Port<u8>,
    config1: PortWriteOnly<u8>,
}

pub struct Ne2000 {
    base_address: u16,
}

impl Ne2000 {
    pub fn new(pci_device: &RwLock<EndpointHeader>) -> Self {
        info!("Configuring PCI registers");
        //Self { base_address }
        //let pci_config_space = pci_bus().config_space();
        let mut pci_device = pci_device.write();

        let bar0 = pci_device
            .bar(0, pci_bus().config_space())
            .expect("Failed to read base address!");

        let base_address = bar0.unwrap_io() as u16;

        info!("NE2000 base address: [0x{:x}]", base_address);
        let mut ne2000 = Self { base_address };
        ne2000
    }

    pub fn read_mac(&self) -> [u8; 6] {
        let mut mac = [0u8; 6];

        unsafe {
            // Define ports
            let mut reset_port = Port::<u8>::new(self.base_address + 0x1F);
            let mut command_port = Port::<u8>::new(self.base_address + 0x00);
            let mut rsar0 = Port::<u8>::new(self.base_address + 0x08);
            let mut rsar1 = Port::<u8>::new(self.base_address + 0x09);
            let mut rbcr0 = Port::<u8>::new(self.base_address + 0x0A);
            let mut rbcr1 = Port::<u8>::new(self.base_address + 0x0B);
            let mut data_port = Port::<u8>::new(self.base_address + 0x10);

            // 1. Reset the NIC
            reset_port.read();

            // 2. Set up Remote DMA to read from address 0x0000
            rsar0.write(0x00);
            rsar1.write(0x00);
            rbcr0.write(6);
            rbcr1.write(0);

            // 3. Issue Remote Read command
            command_port.write(0x0A); // Remote Read

            // 4. Read 6 bytes (MAC address)
            for byte in mac.iter_mut() {
                *byte = data_port.read();
            }
        }

        mac
    }
}
