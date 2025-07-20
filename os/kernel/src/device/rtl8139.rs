use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::BitOr;
use core::{ptr, slice};
use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU8, Ordering};
use bitflags::bitflags;
use log::info;
use nolock::queues::{mpmc, mpsc};
use pci_types::{CommandRegister, EndpointHeader};
use smoltcp::phy;
use smoltcp::phy::{DeviceCapabilities, Medium};
use smoltcp::time::Instant;
use smoltcp::wire::EthernetAddress;
use spin::{Mutex, RwLock};
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::{PhysAddr, VirtAddr};
 
use crate::{apic, interrupt_dispatcher, pci_bus, process_manager, scheduler};
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use crate::memory::{vmm, PAGE_SIZE};

const BUFFER_SIZE: usize = 8 * 1024 + 16 + 1500;
const BUFFER_PAGES: usize = if BUFFER_SIZE % PAGE_SIZE == 0 { BUFFER_SIZE / PAGE_SIZE } else { BUFFER_SIZE / PAGE_SIZE + 1 };
const RECV_QUEUE_CAP: usize = 16;

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

struct ReceiveBuffer {
    index: usize,
    data: Vec<u8>
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
    recv_buffer: Mutex<ReceiveBuffer>,
    send_queue: (Mutex<mpsc::jiffy::Receiver<PhysFrameRange>>, mpsc::jiffy::Sender<PhysFrameRange>),
    recv_buffers_empty: (mpmc::bounded::scq::Receiver<Vec<u8, PacketAllocator>>, mpmc::bounded::scq::Sender<Vec<u8, PacketAllocator>>),
    recv_messages: (mpmc::bounded::scq::Receiver<Vec<u8, PacketAllocator>>, mpmc::bounded::scq::Sender<Vec<u8, PacketAllocator>>)
}

pub struct Rtl8139InterruptHandler {
    device: Arc<Rtl8139>
}

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

impl ReceiveBuffer {
    pub fn new() -> Self {
        let receive_memory = unsafe { vmm::alloc_frames(BUFFER_PAGES) };
        let receive_buffer = unsafe { Vec::from_raw_parts(receive_memory.start.start_address().as_u64() as *mut u8, BUFFER_SIZE, BUFFER_SIZE) };
        Self { index: 0, data: receive_buffer }
    }
}

#[derive(Default)]
pub struct PacketAllocator;

unsafe impl Allocator for PacketAllocator {
    fn allocate(&self, _layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        panic!("PacketAllocator does not support allocate!");
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if layout.size() != PAGE_SIZE {
            panic!("PacketAllocator may only be used with page frames!");
        }

        let start = PhysFrame::from_start_address(PhysAddr::new(ptr.as_ptr() as u64)).expect("PacketAllocator may only be used with page frames!");
        unsafe { vmm::free_frames(PhysFrameRange { start, end: start + 1 }); }
    }
}

pub struct Rtl8139TxToken<'a> {
    device: &'a Rtl8139
}

pub struct Rtl8139RxToken<'a> {
    buffer: Vec<u8, PacketAllocator>,
    device: &'a Rtl8139
}

impl<'a> Rtl8139TxToken<'a> {
    pub fn new(device: &'a Rtl8139) -> Self {
        Self { device }
    }
}

impl<'a> Rtl8139RxToken<'a> {
    pub fn new(buffer: Vec<u8, PacketAllocator>, device: &'a Rtl8139) -> Self {
        Self { buffer, device }
    }
}

impl<'a> phy::TxToken for Rtl8139TxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where F: FnOnce(&mut [u8]) -> R {
        if len > PAGE_SIZE {
            panic!("Packet length may not exceed page size!");
        }

       // Allocate physical memory for the packet (DMA only works with physical addresses)
        let phys_buffer = unsafe { vmm::alloc_frames(1) };
        let phys_start_addr = phys_buffer.start.start_address();
        let pages = PageRange {
            start: Page::from_start_address(VirtAddr::new(phys_start_addr.as_u64())).unwrap(),
            end: Page::from_start_address(VirtAddr::new(phys_buffer.end.start_address().as_u64())).unwrap()
        };

        // Disable caching for allocated buffer
        let kernel_process = process_manager().read().kernel_process().unwrap();
        kernel_process.virtual_address_space.set_flags(pages, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE);

        // Queue physical memory for deallocation after transmission
        self.device.send_queue.1.enqueue(phys_buffer).expect("Failed to enqueue physical buffer!");

        // Let smoltcp write the packet data to the buffer
        let buffer = unsafe { slice::from_raw_parts_mut(phys_buffer.start.start_address().as_u64() as *mut u8, len) };
        let result = f(buffer);

        // Get current transmit descriptor
        let index = self.device.next_transmit_descriptor();
        let mut descriptor = self.device.registers.transmit_descriptors[index].lock();

        // Wait for current descriptor to be available
        while !descriptor.available() {
            scheduler().switch_thread_no_interrupt();
        }

        // Send packet by writing physical address and packet length to transmit registers
        unsafe {
            descriptor.address.write(phys_start_addr.as_u64() as u32);
            descriptor.status.write(buffer.len() as u32);
        }

        result
    }
}

impl<'a> phy::RxToken for Rtl8139RxToken<'a> {
    fn consume<R, F>(mut self, f: F) -> R
    where F: FnOnce(&[u8]) -> R {
        let result = f(&mut self.buffer);
        self.device.recv_buffers_empty.1.try_enqueue(self.buffer).expect("Failed to enqueue used receive buffer!");

        result
    }
}

impl phy::Device for Rtl8139 {
    type RxToken<'a> = Rtl8139RxToken<'a> where Self: 'a;
    type TxToken<'a> = Rtl8139TxToken<'a> where Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let device = unsafe { ptr::from_ref(self).as_ref()? };
        match self.recv_messages.0.try_dequeue() {
            Ok(recv_buf) => Some((Rtl8139RxToken::new(recv_buf, device), Rtl8139TxToken::new(device))),
            Err(_) => None
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        let device = unsafe { ptr::from_ref(self).as_ref()? };
        Some(Rtl8139TxToken::new(device))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1536;
        caps.max_burst_size = Some(1);
        caps.medium = Medium::Ethernet;

        caps
    }
}

impl Rtl8139InterruptHandler {
    pub fn new(device: Arc<Rtl8139>) -> Self {
        Self { device }
    }
}

impl InterruptHandler for Rtl8139InterruptHandler {
    fn trigger(&self) {
        if self.device.registers.interrupt_status.is_locked() {
            panic!("Interrupt status register is locked during interrupt!");
        }

        // Read interrupt status register (Each bit corresponds to an interrupt type or error)
        let mut status_reg = self.device.registers.interrupt_status.lock();
        let status = Interrupt::from_bits_retain(unsafe { status_reg.read() });

        // Check error flags
        if status.contains(Interrupt::TRANSMIT_ERROR) {
            panic!("Transmit failed!");
        } else if status.contains(Interrupt::RECEIVE_ERROR) {
            panic!("Receive failed!");
        }

        // Writing the status register clears all bits.
        // According to the RTL8139 documentation, this is not necessary,
        // but QEMU and some hardware require clearing the interrupt status register.
        // Furthermore, this needs to be done before processing the received packet (https://wiki.osdev.org/RTL8139).
        unsafe { status_reg.write(status.bits()); }

        // Handle transmit by freeing allocated buffers
        if status.contains(Interrupt::TRANSMIT_OK) && !vmm::frame_allocator_locked() {
            let mut queue = self.device.send_queue.0.lock();
            let mut buffer = queue.try_dequeue();
            while buffer.is_ok() {
                unsafe { vmm::free_frames(buffer.unwrap()); }
                buffer = queue.try_dequeue();
            }
        }

        // Handle receive interrupt by processing received packet
        if status.contains(Interrupt::RECEIVE_OK) {
            self.device.process_received_packet();
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
        info!("RTL8139 base address: [0x{base_address:x}]");

        let interrupt = InterruptVector::try_from(pci_device.interrupt(pci_config_space).1 + 32).unwrap();
        let send_queue = mpsc::jiffy::queue();

        let kernel_process = process_manager().read().kernel_process().unwrap();
        let recv_buffers = mpmc::bounded::scq::queue(RECV_QUEUE_CAP);
        for _ in 0..RECV_QUEUE_CAP {
            let phys_frame = unsafe { vmm::alloc_frames(1) };
            let pages = PageRange {
                start: Page::from_start_address(VirtAddr::new(phys_frame.start.start_address().as_u64())).unwrap(),
                end: Page::from_start_address(VirtAddr::new(phys_frame.end.start_address().as_u64())).unwrap()
            };

            kernel_process.virtual_address_space.set_flags(pages, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE);

            let buffer = unsafe { Vec::from_raw_parts_in(phys_frame.start.start_address().as_u64() as *mut u8, PAGE_SIZE, PAGE_SIZE, PacketAllocator::default()) };
            recv_buffers.1.try_enqueue(buffer).expect("Failed to enqueue receive buffer!");
        }

        let mut rtl8139 = Self {
            registers: Registers::new(base_address),
            transmit_index: AtomicU8::new(0),
            interrupt,
            recv_buffer: Mutex::new(ReceiveBuffer::new()),
            send_queue: (Mutex::new(send_queue.0), send_queue.1),
            recv_buffers_empty: recv_buffers,
            recv_messages: mpmc::bounded::scq::queue(RECV_QUEUE_CAP)
        };

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
            rtl8139.registers.receive_buffer_start.write(rtl8139.recv_buffer.lock().data.as_ptr() as u32);
            rtl8139.registers.receive_configuration.write((ReceiveFlag::ACCEPT_PHYSICAL_MATCH | ReceiveFlag::ACCEPT_BROADCAST | ReceiveFlag::WRAP | ReceiveFlag::LENGTH_8K).bits());
            rtl8139.registers.current_read_address.lock().write(0);

            info!("Enabling transmitter/receiver");
            rtl8139.registers.command.write((Command::ENABLE_TRANSMITTER | Command::ENABLE_RECEIVER).bits());
        }

        rtl8139
    }

    pub fn plugin(device: Arc<Rtl8139>) {
        let interrupt = device.interrupt;
        interrupt_dispatcher().assign(device.interrupt, Box::new(Rtl8139InterruptHandler::new(device)));
        apic().allow(interrupt);
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

    fn next_transmit_descriptor(&self) -> usize {
        let index = self.transmit_index.fetch_add(1, Ordering::Relaxed);
        (index % 4) as usize
    }

    fn process_received_packet(&self) {
        if let Some(mut recv_buffer) = self.recv_buffer.try_lock() {
            // Read packet header
            let header = unsafe { (recv_buffer.data.as_ptr().add(recv_buffer.index) as *const PacketHeader).read() };

            // Check if packet is valid and update index accordingly
            let status = ReceiveStatus::from_bits_retain(header.status);
            if status.contains(ReceiveStatus::OK) {
                // Calculate start and end of received message
                let msg_start = recv_buffer.index + size_of::<PacketHeader>();
                let msg_end = msg_start + header.length as usize;

                // Update device state for next incoming message
                recv_buffer.index += header.length as usize + size_of::<PacketHeader>(); // Add packet length
                recv_buffer.index = (recv_buffer.index + 3) & !3; // Align to 4 bytes
                if recv_buffer.index >= 8192 {
                    recv_buffer.index %= 8192; // Wrap around buffer
                }

                let mut read_addr_register = self.registers.current_read_address.try_lock().expect("Current read address register is locked during packet processing");
                unsafe { read_addr_register.write(recv_buffer.index as u32) };

                // Copy message to new buffer and enqueue for processing
                if let Ok(mut target) = self.recv_buffers_empty.0.try_dequeue() {
                    let src = &recv_buffer.data[msg_start..msg_end];
                    target[0..src.len()].copy_from_slice(src);

                    let _ = self.recv_messages.1.try_enqueue(target);
                }
            }
        } else {
            panic!("Receive buffer is locked during packet processing!");
        }
    }
}