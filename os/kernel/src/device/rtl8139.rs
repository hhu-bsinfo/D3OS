use core::ops::BitOr;
use bitflags::bitflags;
use log::info;
use pci_types::{CommandRegister, EndpointHeader};
use smoltcp::wire::EthernetAddress;
use spin::{Mutex, RwLock};
use x86_64::instructions::port::PortReadOnly;
use crate::pci_bus;

bitflags! {
    pub struct Command: u8 {
        const BUFFER_EMPTY = 0x01;
        const ENABLE_TRANSMITTER = 0x04;
        const ENABLE_RECEIVER = 0x08;
        const RESET = 0x10;
    }
}

bitflags! {
    pub struct Interrupt: u16 {
        const RECEIVE_OK = 0x0001;
        const RECEIVE_ERROR = 0x0002;
        const TRANSMIT_OK = 0x0004;
        const TRANSMIT_ERROR = 0x0008;
        const RX_BUFFER_OVERFLOW = 0x0010;
        const PACKET_UNDERRUN_LINK_CHANGE = 0x0020;
        const RX_FIFO_OVERFLOW = 0x0040;
        const CABLE_LENGTH_CHANGE = 0x2000;
        const TIMEOUT = 0x4000;
        const SYSTEM_ERROR = 0x8000;
    }
}

bitflags! {
    pub struct ReceiveFlag: u32 {
        const ACCEPT_ALL = 0x0001;
        const ACCEPT_PHYSICAL_MATCH = 0x0002;
        const ACCEPT_MULTICAST = 0x0004;
        const ACCEPT_BROADCAST = 0x0008;
        const ACCEPT_RUNT = 0x0010;
        const ACCEPT_ERROR = 0x0020;
        const WRAP = 0x0080;
        const LENGTH_8K = 0x0000;
        const LENGTH_16K = 0x0800;
        const LENGTH_32K = 0x1000;
        const LENGTH_64K = 0x1800;
    }
}

bitflags! {
    pub struct TransmitStatus: u32 {
        const OWN = 0x2000;
        const FIFO_UNDERRUN = 0x4000;
        const TRANSMIT_STATUS_OK = 0x8000;
        const EARLY_TX_THRESHOLD = 0x10000;
        const TRANSMIT_STATUS_ABORT = 0x40000000;
        const CARRIER_SENSE_LOST = 0x80000000;
    }
}

bitflags! {
    pub struct ReceiveStatus: u16 {
        const OK = 0x0001;
        const FRAME_ALIGNMENT_ERROR = 0x0002;
        const CHECKSUM_ERROR = 0x0004;
        const LONG_PACKET = 0x0008;
        const RUNT_PACKET = 0x0010;
        const INVALID_SYMBOL = 0x0020;
        const BROADCAST = 0x2000;
        const PHYSICAL_ADDRESS = 0x4000;
        const MULTICAST = 0x8000;
    }
}

#[repr(C, packed)]
struct PacketHeader {
    status: u16,
    length: u16
}

struct Registers {
    id: Mutex<(PortReadOnly<u8>, PortReadOnly<u8>, PortReadOnly<u8>, PortReadOnly<u8>, PortReadOnly<u8>, PortReadOnly<u8>)>,
    transmit_status: PortReadOnly<u16>,
    transmit_address: PortReadOnly<u32>,
    receive_buffer_start: PortReadOnly<u32>,
    command: PortReadOnly<u8>,
    current_read_address: PortReadOnly<u32>,
    interrupt_mask: PortReadOnly<u16>,
    interrupt_status: PortReadOnly<u16>,
    receive_configuration: PortReadOnly<u32>,
    config1: PortReadOnly<u8>,
}

pub struct Rtl8139 {
    registers: Registers
}

impl Rtl8139 {
    pub fn new(pci_device: &RwLock<EndpointHeader>) -> Self {
        let pci_config_space = pci_bus().config_space();
        let mut pci_device = pci_device.write();

        // Make sure bus master and memory space are enabled for MMIO register access
        pci_device.update_command(pci_config_space, |command| {
            command.bitor(CommandRegister::BUS_MASTER_ENABLE | CommandRegister::MEMORY_ENABLE)
        });

        // Read register base address from BAR0
        let bar0 = pci_device.bar(0, pci_bus().config_space()).expect("Failed to read base address!");
        let base_address = bar0.unwrap_io() as u16;
        info!("RTL8139 base address: [0x{:x}]", base_address);

        let registers = Registers {
            id: Mutex::new((
                PortReadOnly::new(base_address + 0x00),
                PortReadOnly::new(base_address + 0x01),
                PortReadOnly::new(base_address + 0x02),
                PortReadOnly::new(base_address + 0x03),
                PortReadOnly::new(base_address + 0x04),
                PortReadOnly::new(base_address + 0x05),
            )),
            transmit_status: PortReadOnly::new(base_address + 0x10),
            transmit_address: PortReadOnly::new(base_address + 0x20),
            command: PortReadOnly::new(base_address + 0x37),
            receive_buffer_start: PortReadOnly::new(base_address + 0x30),
            current_read_address: PortReadOnly::new(base_address + 0x38),
            interrupt_mask: PortReadOnly::new(base_address + 0x3c),
            interrupt_status: PortReadOnly::new(base_address + 0x3e),
            receive_configuration: PortReadOnly::new(base_address + 0x44),
            config1: PortReadOnly::new(base_address + 0x52),
        };

        Self { registers }
    }

    pub fn read_mac_address(&self) -> EthernetAddress {
        let mut id_registers = self.registers.id.lock();

        unsafe {
            let mac = [
                id_registers.0.read(),
                id_registers.1.read(),
                id_registers.2.read(),
                id_registers.3.read(),
                id_registers.4.read(),
                id_registers.5.read(),
            ];

            EthernetAddress::from_bytes(&mac)
        }
    }
}