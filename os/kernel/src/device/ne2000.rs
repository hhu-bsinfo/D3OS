use crate::{apic, interrupt_dispatcher, pci_bus, process_manager, scheduler};
use bitflags::bitflags;
use log::info;
use pci_types::{CommandRegister, EndpointHeader};
use smoltcp::wire::EthernetAddress;
use spin::{Mutex, RwLock};
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

static RESET: u8 = 0x1F;

bitflags! {
    pub struct PageRegisters :u16 {
        const COMMAND = 0x00; // R/W Command for Pages 0, 1, 2
        const IOPORT = 0x10;
        const P1_PAR0 = 0x01;
        const P1_PAR1 = 0x02;
        const P1_PAR2 = 0x03;
        const P1_PAR3 = 0x04;
        const P1_PAR4 = 0x05;
        const P1_PAR5 = 0x06;
    }
}

bitflags! {
    pub struct InterruptFlags :u8 {
        const ISR_RST = 0x80;
    }
}

bitflags! {
    pub struct Command :u8 {
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
        let pci_device = pci_device.write();

        let bar0 = pci_device
            .bar(0, pci_bus().config_space())
            .expect("Failed to read base address!");

        let base_address = bar0.unwrap_io() as u16;
        let ne2000 = Self { base_address };
        info!("NE2000 base address: [0x{:x}]", base_address);

        ne2000
    }

    pub fn read_mac(&self) -> [u8; 6] {
        //pub fn read_mac(&self) -> EthernetAddress {
        let mut mac = [0u8; 6];

        unsafe {
            // Define ports

            // reset port
            let mut reset_port = Port::<u8>::new(self.base_address + RESET as u16);
            let mut command_port = Port::<u8>::new(self.base_address + 0x00);
            let mut rsar0 = Port::<u8>::new(self.base_address + 0x08);
            let mut rsar1 = Port::<u8>::new(self.base_address + 0x09);
            let mut rbcr0 = Port::<u8>::new(self.base_address + 0x0A);
            let mut rbcr1 = Port::<u8>::new(self.base_address + 0x0B);
            let mut data_port = Port::<u8>::new(self.base_address + 0x10);

            //Reset the NIC
            // Clears the Registers CR, ISR, IMR, DCR, TCR (see NS32490D.pdf, p.29, 11.0 Initialization Procedure)
            // this ensures, that the Registers are cleared and no undefined behavior can happen
            info!("Resetting Device NE2000");
            reset_port.read();

            //Set up Remote DMA to read from address 0x0000
            // RSAR0,1 : points to the start of the block of data to be transfered
            rsar0.write(0x00);
            rsar1.write(0x00);

            //RBCR0,1 : indicates the length of the block in bytes
            // MAC address has length of 6 Bytes
            rbcr0.write(6);
            rbcr1.write(0);

            //Issue Remote Read command
            // Command Port is 8 Bits and has the following structure
            // |PS1|PS0|RD2|RD1|RD0|TXP|STA|STP|
            // 0x0A => 0000 1010
            // STA : Start the NIC
            // RD0: Remote Read
            //PS0, PS1 : access Register Page 0
            // changed to 0x4A, because PARs are on Page 1, but it was set to Page 0, but somehow worked
            // edit: some ne2000 clones do a reset at the beginning and copy the MAC from PAR0-5 into the ring buffer at address 0x00
            // The ne2000 memory is accessed through the data port of
            // the asic (offset 0) after setting up a remote-DMA transfer.
            // Both byte and word accesses are allowed.
            // The first 16 bytes contains the MAC address at even locations,
            //command_port.write(0x0A);
            //command_port.write(0x20);
            let cr: u8 = unsafe { command_port.read() };
            info!("Page: ({}) ", cr);

            for _ in 0..6 {
                let _ = data_port.read();
            } // discard

            //Read 6 bytes (MAC address)
            for byte in mac.iter_mut() {
                *byte = data_port.read();
            }
            //EthernetAddress::from_bytes(&mac)
            let address1 = EthernetAddress::from_bytes(&mac);
            info!("before ({})", address1);

            command_port.write(0x40);

            let mut par_ports: [Port<u8>; 6] = [
                Port::new(self.base_address + 0x01),
                Port::new(self.base_address + 0x02),
                Port::new(self.base_address + 0x03),
                Port::new(self.base_address + 0x04),
                Port::new(self.base_address + 0x05),
                Port::new(self.base_address + 0x06),
            ];
            for (i, port) in par_ports.iter_mut().enumerate() {
                mac[i] = port.read();
            }

            unsafe {
                let mut command_port = Port::<u8>::new(self.base_address + 0x00);
                let cr = command_port.read();
                let ps = (cr >> 6) & 0b11;

                match ps {
                    0 => info!("Currently on Page 0"),
                    1 => info!("Currently on Page 1"),
                    2 => info!("Currently on Page 2"),
                    3 => info!("Currently on Page 3"),
                    _ => unreachable!(),
                }
            }

            let address2 = EthernetAddress::from_bytes(&mac);
            info!("after ({})", address2);
        }
        mac
    }
}
