use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ops::BitOr;
use core::sync::atomic::{AtomicU8, Ordering};
use bitflags::bitflags;
use log::info;
use nolock::queues::mpsc;
use pci_types::{CommandRegister, EndpointHeader};
use smoltcp::wire::EthernetAddress;
use spin::{Mutex, RwLock};
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};
use x86_64::structures::paging::{Page, PageTableFlags};
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::VirtAddr;
use crate::{apic, interrupt_dispatcher, pci_bus, process_manager, rtl8139, scheduler};
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use crate::memory::{physical, PAGE_SIZE};

const BUFFER_SIZE: usize = 8 * 1024 + 16 + 1500;
const BUFFER_PAGES: usize = if BUFFER_SIZE % PAGE_SIZE == 0 { BUFFER_SIZE / PAGE_SIZE } else { BUFFER_SIZE / PAGE_SIZE + 1 };

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

struct TransmitDescriptor {
    status: Port<u32>,
    address: PortWriteOnly<u32>
}

struct Receiver {
    index: usize,
    buffer: Vec<u8>
}

struct Registers {
    id: Mutex<(PortReadOnly<u8>, PortReadOnly<u8>, PortReadOnly<u8>, PortReadOnly<u8>, PortReadOnly<u8>, PortReadOnly<u8>)>,
    transmit_descriptors: [Mutex<TransmitDescriptor>; 4],
    receive_buffer_start: PortWriteOnly<u32>,
    command: Port<u8>,
    current_read_address: Mutex<Port<u32>>,
    interrupt_mask: PortWriteOnly<u16>,
    interrupt_status: Mutex<Port<u16>>,
    receive_configuration: PortWriteOnly<u32>,
    config1: PortWriteOnly<u8>,
}

pub struct Rtl8139 {
    registers: Registers,
    transmit_index: AtomicU8,
    interrupt: InterruptVector,
    send_queue: (Mutex<mpsc::jiffy::Receiver<PhysFrameRange>>, mpsc::jiffy::Sender<PhysFrameRange>),
    receiver: Mutex<Receiver>
}

#[derive(Default)]
pub struct Rtl8139InterruptHandler;

impl Registers {
    fn new(base_address: u16) -> Self {
        Self {
            id: Mutex::new((
                PortReadOnly::new(base_address + 0x00),
                PortReadOnly::new(base_address + 0x01),
                PortReadOnly::new(base_address + 0x02),
                PortReadOnly::new(base_address + 0x03),
                PortReadOnly::new(base_address + 0x04),
                PortReadOnly::new(base_address + 0x05),
            )),
            transmit_descriptors: [Mutex::new(TransmitDescriptor::new(base_address, 0)),
                                   Mutex::new(TransmitDescriptor::new(base_address, 1)),
                                   Mutex::new(TransmitDescriptor::new(base_address, 2)),
                                   Mutex::new(TransmitDescriptor::new(base_address, 3))],
            command: Port::new(base_address + 0x37),
            receive_buffer_start: PortWriteOnly::new(base_address + 0x30),
            current_read_address: Mutex::new(Port::new(base_address + 0x38)),
            interrupt_mask: PortWriteOnly::new(base_address + 0x3c),
            interrupt_status: Mutex::new(Port::new(base_address + 0x3e)),
            receive_configuration: PortWriteOnly::new(base_address + 0x44),
            config1: PortWriteOnly::new(base_address + 0x52),
        }
    }
}

impl TransmitDescriptor {
    fn new(base_address: u16, index: u8) -> Self {
        assert!(index < 4, "Transmit descriptor index out of bounds!");

        Self {
            status: Port::new(base_address + 0x10 + index as u16 * 4),
            address: PortWriteOnly::new(base_address + 0x20 + index as u16 * 4)
        }
    }

    fn available(&mut self) -> bool {
        let status = unsafe { self.status.read() };
        TransmitStatus::from_bits_retain(status).contains(TransmitStatus::OWN)
    }
}

impl Receiver {
    pub fn new() -> Self {
        let receive_memory = physical::alloc(BUFFER_PAGES);
        let receive_buffer = unsafe { Vec::from_raw_parts(receive_memory.start.start_address().as_u64() as *mut u8, BUFFER_SIZE, BUFFER_SIZE) };

        Self { index: 0, buffer: receive_buffer }
    }
}

impl InterruptHandler for Rtl8139InterruptHandler {
    fn trigger(&mut self) {
        if let Some(rtl8139) = rtl8139() {
            if rtl8139.registers.interrupt_status.is_locked() {
                panic!("Interrupt status register is locked during interrupt!");
            }

            let mut status_reg = rtl8139.registers.interrupt_status.lock();
            let status = Interrupt::from_bits_retain(unsafe { status_reg.read() });

            if status.contains(Interrupt::TRANSMIT_OK) && !physical::allocator_locked() {
                let mut queue = rtl8139.send_queue.0.lock();
                let mut buffer = queue.try_dequeue();
                while buffer.is_ok() {
                    unsafe { physical::free(buffer.unwrap()) };
                    buffer = queue.try_dequeue();
                }
            }

            if status.contains(Interrupt::RECEIVE_OK) {
                rtl8139.process_received_packet();
            }

            if status.contains(Interrupt::TRANSMIT_ERROR) {
                panic!("Transmit failed!");
            }

            unsafe { status_reg.write(status.bits()); }
        }
    }
}

impl Rtl8139 {
    pub fn new(pci_device: &RwLock<EndpointHeader>) -> Self {
        info!("Configuring PCI registers");
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

        let interrupt = InterruptVector::try_from(pci_device.interrupt(pci_config_space).1 + 32).unwrap();
        let send_queue = mpsc::jiffy::queue();

        let mut rtl8139 = Self { registers: Registers::new(base_address), transmit_index: AtomicU8::new(0), interrupt, send_queue: (Mutex::new(send_queue.0), send_queue.1), receiver: Mutex::new(Receiver::new()) };

        unsafe {
            info!("Powering on device");
            rtl8139.registers.config1.write(0x00);

            info!("Performing software reset");
            rtl8139.registers.command.write(Command::RESET.bits());

            // Wait for device to unset RESET bit
            while Command::from_bits_retain(rtl8139.registers.command.read()).contains(Command::RESET) {
                scheduler().sleep(1);
            }

            info!("Masking interrupts");
            rtl8139.registers.interrupt_mask.write((Interrupt::RECEIVE_OK | Interrupt::RECEIVE_ERROR | Interrupt::TRANSMIT_OK | Interrupt::TRANSMIT_ERROR).bits());

            info!("Configuring receive buffer");
            rtl8139.registers.receive_buffer_start.write(rtl8139.receiver.lock().buffer.as_ptr() as u32);
            rtl8139.registers.receive_configuration.write((ReceiveFlag::ACCEPT_PHYSICAL_MATCH | ReceiveFlag::ACCEPT_BROADCAST | ReceiveFlag::WRAP | ReceiveFlag::LENGTH_8K).bits());
            rtl8139.registers.current_read_address.lock().write(0);

            info!("Enabling transmitter/receiver");
            rtl8139.registers.command.write((Command::ENABLE_TRANSMITTER | Command::ENABLE_RECEIVER).bits());
        }

        return rtl8139;
    }

    pub fn plugin(&self) {
        interrupt_dispatcher().assign(self.interrupt, Box::new(Rtl8139InterruptHandler::default()));
        apic().allow(self.interrupt);
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

    pub fn send(&self, data: &[u8]) {
        // Allocate physical memory for the packet (DMA only works with physical addresses)
        let page_count = if data.len() % 4096 == 0 { data.len() / 4096 } else { data.len() / 4096 + 1 };
        let phys_buffer = physical::alloc(page_count);
        let phys_start_addr = phys_buffer.start.start_address();
        let pages = PageRange {
            start: Page::from_start_address(VirtAddr::new(phys_start_addr.as_u64())).unwrap(),
            end: Page::from_start_address(VirtAddr::new(phys_buffer.end.start_address().as_u64())).unwrap()
        };

        // Queue physical memory for deallocation after transmission
        self.send_queue.1.enqueue(phys_buffer).expect("Failed to enqueue physical buffer!");

        // Disable caching for allocated buffer
        let kernel_process = process_manager().read().kernel_process().unwrap();
        kernel_process.address_space().set_flags(pages, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE);

        // Copy data to physical memory
        unsafe {
            data.as_ptr().copy_to(phys_start_addr.as_u64() as *mut u8, data.len());
        }

        // Get current transmit descriptor
        let index = self.next_transmit_descriptor();
        let mut descriptor = self.registers.transmit_descriptors[index].lock();

        // Wait for current descriptor to be available
        while !descriptor.available() {
            scheduler().switch_thread_no_interrupt();
        }

        // Send packet by writing physical address and packet length to transmit registers
        unsafe {
            descriptor.address.write(phys_start_addr.as_u64() as u32);
            descriptor.status.write(data.len() as u32);
        }
    }

    fn next_transmit_descriptor(&self) -> usize {
        let index = self.transmit_index.fetch_add(1, Ordering::Relaxed);
        (index % 4) as usize
    }

    fn process_received_packet(&self) {
        if let Some(mut receiver) = self.receiver.try_lock() {
            // Read packet header
            let header = unsafe { (receiver.buffer.as_ptr().add(receiver.index) as *const PacketHeader).read() };

            // Check if packet is valid and update index accordingly
            let status = ReceiveStatus::from_bits_retain(header.status);
            if status.contains(ReceiveStatus::OK) {
                receiver.index += header.length as usize + size_of::<PacketHeader>(); // Add packet length
                receiver.index = (receiver.index + 3) & !3; // Align to 4 bytes
                if receiver.index >= 8192 {
                    receiver.index %= 8192; // Wrap around buffer
                }

                if let Some(mut read_addr_register) = self.registers.current_read_address.try_lock() {
                    unsafe { read_addr_register.write(receiver.index as u32) };
                } else {
                    panic!("Current read address register is locked during packet processing!");
                }
            }
        } else {
            panic!("Receiver is locked during packet processing!");
        }
    }
}