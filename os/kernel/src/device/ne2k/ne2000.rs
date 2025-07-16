// =============================================================================
// FILE        : ne2000.rs
// AUTHOR      : Johann Spenrath
// DESCRIPTION : Main file for the NE2000 driver
// =============================================================================
//
//
//
// TODO:
//
// NOTES:
//
// =============================================================================
//
//
//
//
//
//
//
//
//
//
//
// =============================================================================
// DEPENDENCIES:
// =============================================================================
use crate::device::ne2k::register_flags::ReceiveStatusRegister;
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::memory::{PAGE_SIZE, frames};
use crate::process::thread::Thread;
use crate::{apic, device, interrupt_dispatcher, pci_bus, process_manager, scheduler};
use acpi::platform::interrupt;
use core::mem;
use core::ops::BitOr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use core::{ptr, slice};
use log::info;
// for allocator impl
use alloc::boxed::Box;
// for allocator impl
use crate::interrupt::interrupt_handler::InterruptHandler;
use core::ptr::NonNull;
use spin::{Mutex, RwLock};
// let smoltcp handle the packet
// TODO: check network/mod.rs for the handling of the packet
// probably an interrupt handler has to be assigned, check this

// lock free algorithms and datastructes
// queues: different queue implementations
// mpsc : has the jiffy queue ; lock-free unbounded, for send
// mpmpc : multiple producers, multiple consumers, for receive
use nolock::queues::{mpmc, mpsc};

use pci_types::EndpointHeader;
// smoltcp provides a full network stack for creating packets, sending, receiving etc.
use alloc::sync::Arc;
use alloc::vec::Vec;
use smoltcp::wire::EthernetAddress;

// for writing to the registers
use alloc::str;
use alloc::string::String;
use x86_64::VirtAddr;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};

// super looks in a relative path for other modules
use super::register_flags::{
    CR, DataConfigurationRegister, InterruptMaskRegister, InterruptStatusRegister,
    ReceiveConfigurationRegister, TransmitConfigurationRegister,
};

use super::network_stack::*;

// =============================================================================

// Atomically store a pointer to Ne2000 when initializing.
// and load the pointer inside the interrupt-handling thread.
// This ensures no partial reads or writes happen.
// It's safe to use across threads because atomic pointer operations are guaranteed
// to be lock-free
static NE2000_PTR: AtomicPtr<Ne2000> = AtomicPtr::new(core::ptr::null_mut());

const RECV_QUEUE_CAP: usize = 16;

const DISPLAY_RED: &'static str = "\x1b[1;31m";
static MINIMUM_ETHERNET_PACKET_SIZE: u8 = 64;
static MAXIMUM_ETHERNET_PACKET_SIZE: u32 = 1522;
static mut CURRENT_NEXT_PAGE_POINTER: u8 = 0x00;

// Buffer Start Page for the transmitted pages
static TRANSMIT_START_PAGE: u8 = 0x40;

// Reception Buffer Ring Start Page
// http://www.osdever.net/documents/WritingDriversForTheDP8390.pdf
// Page 4 PSTART
static RECEIVE_START_PAGE: u8 = 0x46;

//Reception Buffer Ring End
//P.4 PSTOP http://www.osdever.net/documents/WritingDriversForTheDP8390.pdf
static RECEIVE_STOP_PAGE: u8 = 0x80;

pub struct ParRegisters {
    id: Mutex<(
        PortReadOnly<u8>,
        PortReadOnly<u8>,
        PortReadOnly<u8>,
        PortReadOnly<u8>,
        PortReadOnly<u8>,
        PortReadOnly<u8>,
    )>,
}

impl ParRegisters {
    pub fn new(base_address: u16) -> Self {
        Self {
            id: Mutex::new((
                PortReadOnly::new(base_address + 0x01),
                PortReadOnly::new(base_address + 0x02),
                PortReadOnly::new(base_address + 0x03),
                PortReadOnly::new(base_address + 0x04),
                PortReadOnly::new(base_address + 0x05),
                PortReadOnly::new(base_address + 0x06),
            )),
        }
    }
}

pub struct Registers {
    reset_port: Port<u8>,
    command_port: Port<u8>,
    rsar0: Port<u8>,
    rsar1: Port<u8>,
    rbcr0: Port<u8>,
    rbcr1: Port<u8>,
    data_port: Port<u8>,
    // add Mutex (05.07.2025)
    isr_port: Mutex<Port<u8>>,
    imr_port: Mutex<Port<u8>>,
    rst_port: Port<u8>,
    dcr_port: Port<u8>,
    tcr_port: Port<u8>,
    rcr_port: Port<u8>,
    tpsr_port: Port<u8>,
    pstart_port: Port<u8>,
    pstop_port: Port<u8>,
    bnry_port: Port<u8>,
    par_0: Port<u8>,
    par_1: Port<u8>,
    par_2: Port<u8>,
    par_3: Port<u8>,
    par_4: Port<u8>,
    par_5: Port<u8>,
    curr: Port<u8>,
    mar0: Port<u8>,
    mar1: Port<u8>,
    mar2: Port<u8>,
    mar3: Port<u8>,
    mar4: Port<u8>,
    mar5: Port<u8>,
    mar6: Port<u8>,
    mar7: Port<u8>,
    crda0_p0: Port<u8>,
    crda1_p0: Port<u8>,
    tpsr: Port<u8>,
    tbcr0_p0: Port<u8>,
    tbcr1_p0: Port<u8>,
}

impl Registers {
    pub fn new(base_address: u16) -> Self {
        // TODO: replace hex with Register names defined in a different struct for better readibility
        Self {
            // Adress for reseting the device
            // TODO: add OSDEV WIKI reference
            reset_port: Port::new(base_address + 0x1F),
            // command Port for controlling the CR Register
            //(starting, stopping the nic, switching between pages)
            command_port: Port::new(base_address + 0x00),
            rsar0: Port::new(base_address + 0x08),
            rsar1: Port::new(base_address + 0x09),
            rbcr0: Port::new(base_address + 0x0A),
            rbcr1: Port::new(base_address + 0x0B),
            // data port (or i/o port for reading received data)
            data_port: Port::new(base_address + 0x10),
            isr_port: Mutex::new(Port::new(base_address + 0x07)),
            rst_port: Port::new(base_address + 0x80),
            imr_port: Mutex::new(Port::new(base_address + 0x0F)),
            dcr_port: Port::new(base_address + 0x0E),
            tcr_port: Port::new(base_address + 0x0D),
            rcr_port: Port::new(base_address + 0x0C),
            tpsr_port: Port::new(base_address + 0x04),
            pstart_port: Port::new(base_address + 0x01),
            pstop_port: Port::new(base_address + 0x02),
            bnry_port: Port::new(base_address + 0x03),
            par_0: Port::new(base_address + 0x01),
            par_1: Port::new(base_address + 0x02),
            par_2: Port::new(base_address + 0x03),
            par_3: Port::new(base_address + 0x04),
            par_4: Port::new(base_address + 0x05),
            par_5: Port::new(base_address + 0x06),
            curr: Port::new(base_address + 0x07),
            mar0: Port::new(base_address + 0x08),
            mar1: Port::new(base_address + 0x09),
            mar2: Port::new(base_address + 0x0A),
            mar3: Port::new(base_address + 0x0B),
            mar4: Port::new(base_address + 0x0C),
            mar5: Port::new(base_address + 0x0D),
            mar6: Port::new(base_address + 0x0E),
            mar7: Port::new(base_address + 0x0F),
            crda0_p0: Port::new(base_address + 0x08),
            crda1_p0: Port::new(base_address + 0x09),
            tpsr: Port::new(base_address + 0x04),
            tbcr0_p0: Port::new(base_address + 0x05),
            tbcr1_p0: Port::new(base_address + 0x06),
        }
    }

    fn read_isr(&self) -> u8 {
        unsafe { self.isr_port.lock().read() }
    }
    pub fn write_imr(&self, val: u8) {
        unsafe { self.imr_port.lock().write(val) }
    }
}

// 0x80 - 0x46 = 0x58 = 58 pages
// total buffer size = 58 * 256 Bytes  = 14.KiB

// The Structure of the PacketHeader is definied in the datasheet
// Header is 4 KB
// TODO: add reference
// receive status : holds the content of the Receive Status Register
// next_packet : Pointer, which holds the next ringbuffer address
// length : length of the received data

#[repr(C)]
struct PacketHeader {
    receive_status: u8,
    next_packet: u8,
    length: u16,
}

pub struct Interrupts {
    ovw: Mutex<bool>,
    rcv: Mutex<bool>,
}

// par_registers : store the MAC ADDRESS
// send_queue: needed for packet transmission process in smoltcp
// TODO: implement receive queue, see rtl8139
// EXAMPLE for a sender and receiver
//#![feature(mpmc_channel)]
//
//use std::thread;
//use std::sync::mpmc::channel;
//
//// Create a simple streaming channel
//let (tx, rx) = channel();
//thread::spawn(move || {
//    tx.send(10).unwrap();
//});
//assert_eq!(rx.recv().unwrap(), 10);
pub struct Ne2000 {
    base_address: u16,
    pub registers: Registers,
    par_registers: ParRegisters,
    // physical memory ranges, that need transmitting
    // in TxToken consume the outgoing packet gets loaded into the buffer
    pub send_queue: (
        Mutex<mpsc::jiffy::Receiver<PhysFrameRange>>,
        mpsc::jiffy::Sender<PhysFrameRange>,
    ),
    receive_buffer: Mutex<ReceiveBuffer>,
    // pre-allocated, empty Vec<u8> buffers which get filled with incoming packets
    pub receive_buffers_empty: (
        mpmc::bounded::scq::Receiver<Vec<u8, PacketAllocator>>,
        // Sender send data to a set of Receivers
        mpmc::bounded::scq::Sender<Vec<u8, PacketAllocator>>,
    ),
    // contain the actual data which is received
    pub receive_messages: (
        mpmc::bounded::scq::Receiver<Vec<u8, PacketAllocator>>,
        mpmc::bounded::scq::Sender<Vec<u8, PacketAllocator>>,
    ),
    interrupt: InterruptVector,
    interrupts: Interrupts,
    pub(crate) rcv: AtomicBool,
}

impl Ne2000 {
    pub fn new(pci_device: &RwLock<EndpointHeader>) -> Self {
        info!("Configuring PCI registers");
        //Self { base_address }
        //let pci_config_space = pci_bus().config_space();
        let pci_device = pci_device.write();
        let pci_config_space = pci_bus().config_space();

        let bar0 = pci_device
            .bar(0, pci_bus().config_space())
            .expect("Failed to read base address!");
        let base_address = bar0.unwrap_io() as u16;
        info!("NE2000 base address: [0x{:x}]", base_address);

        // mpsc (multiple-producer, single-consumer)
        // FIFO queue implementation

        // queue creates a new empty queue and returns (Receiver, Sender)
        // send_queue.enqueue(13) -> enque data
        // see: https://docs.rs/nolock/latest/nolock/queues/mpsc/jiffy/index.html
        let send_queue = mpsc::jiffy::queue();
        // enable interrupts
        let interrupt =
            InterruptVector::try_from(pci_device.interrupt(pci_config_space).1 + 32).unwrap();
        let kernel_process = process_manager().read().kernel_process().unwrap();
        let recv_buffers = mpmc::bounded::scq::queue(RECV_QUEUE_CAP);
        for _ in 0..RECV_QUEUE_CAP {
            let phys_frame = frames::alloc(1);
            let pages = PageRange {
                start: Page::from_start_address(VirtAddr::new(
                    phys_frame.start.start_address().as_u64(),
                ))
                .unwrap(),
                end: Page::from_start_address(VirtAddr::new(
                    phys_frame.end.start_address().as_u64(),
                ))
                .unwrap(),
            };

            kernel_process.virtual_address_space.set_flags(
                pages,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE,
            );

            let buffer = unsafe {
                Vec::from_raw_parts_in(
                    phys_frame.start.start_address().as_u64() as *mut u8,
                    PAGE_SIZE,
                    PAGE_SIZE,
                    PacketAllocator::default(),
                )
            };
            recv_buffers
                .1
                .try_enqueue(buffer)
                .expect("Failed to enqueue receive buffer!");
        }

        let interrupts = Interrupts {
            ovw: Mutex::new(false),
            rcv: Mutex::new(false),
        };

        // construct the ne2000 and return it at the end of the
        // initialization
        let mut ne2000 = Self {
            registers: Registers::new(base_address),
            base_address: base_address,
            par_registers: ParRegisters::new(base_address),
            send_queue: (Mutex::new(send_queue.0), send_queue.1),
            receive_buffers_empty: recv_buffers,
            receive_buffer: Mutex::new(ReceiveBuffer::new()),
            receive_messages: mpmc::bounded::scq::queue(RECV_QUEUE_CAP),
            interrupt,
            interrupts,
            rcv: AtomicBool::new(false),
        };

        info!("\x1b[1;31mPowering on device");
        unsafe {
            info!("\x1b[1;31mResetting Device NE2000");

            //Reset the NIC
            // Clears the Registers CR, ISR, IMR, DCR, TCR (see NS32490D.pdf, p.29, 11.0 Initialization Procedure)
            // this ensures, that the Registers are cleared and no undefined behavior can happen

            // From C++ Ne2000
            /* Wait until Reset Status is 0 */
            //while(!(baseRegister.readByte(P0_ISR) & ISR_RST)) {
            //Util::Async::Thread::sleep(Util::Time::Timestamp::ofMilliseconds(1));
            //}
            // Wait for the reset to complete
            //reset_port.write(0x00);

            // just doing the read operation enables the reset, a write is not necessary, but the bits dont get set correctly
            // see spec in PDF
            //TODO:, add comments what registers are affected and which bits are set
            let reset_value = ne2000.registers.reset_port.read();
            ne2000.registers.reset_port.write(reset_value);

            // bitwise and operation, checks if highest bit is set
            // if register content equals 0, reset was successful
            while (ne2000.registers.read_isr() & 0x80) == 0 {
                info!("Reset in Progress");
            }
            info!("\x1b[1;31mNe2000 reset complete");

            info!("\x1b[1;31mInitializing Registers of Device Ne2000");

            // Initialize CR Register
            // Switch to Page0 , stop DMA and set the NIC in Stop mode
            ne2000
                .registers
                .command_port
                .write((CR::STOP_DMA | CR::STP | CR::PAGE_0).bits());

            // Initialize DCR Register
            // from the NS32490D.pdf :
            // Register is used to program the NIC for 8- or 16-bit memory interface,
            // select byte ordering in 16-bit applications and
            // establish FIFO threshholds. The DCR must be initialized prior to loading the Remote Byte Count Registers.
            // TODO: add reference
            // Command Register at Page 0 at this point
            ne2000.registers.dcr_port.write(
                (DataConfigurationRegister::DCR_AR
                    | DataConfigurationRegister::DCR_FT1
                    | DataConfigurationRegister::DCR_LS)
                    .bits(),
            );

            // clear RBCR1,0
            //RBCR0,1 : indicates the length of the block in bytes
            // MAC address has length of 6 Bytes
            ne2000.registers.rbcr0.write(0);
            ne2000.registers.rbcr1.write(0);

            // initialize RCR
            // determines operation of the NIC during reception of a packet and is used to program what types of packets to
            // accept.
            ne2000.registers.rcr_port.write(
                (ReceiveConfigurationRegister::RCR_AR
                    | ReceiveConfigurationRegister::RCR_AB
                    | ReceiveConfigurationRegister::RCR_AM)
                    .bits(),
            );

            // Place the NIC in Loopback Mode (Mode 1)
            // TODO: add reference from the handbook
            ne2000
                .registers
                .tcr_port
                .write(TransmitConfigurationRegister::TCR_LB0.bits());

            // initialize the NIC's buffer
            // pstart and pstop define the size of the buffer (pstop - pstart = buffer size )
            ne2000.registers.tpsr_port.write(TRANSMIT_START_PAGE);
            ne2000.registers.pstart_port.write(RECEIVE_START_PAGE);
            ne2000.registers.bnry_port.write(RECEIVE_START_PAGE + 1);
            ne2000.registers.pstop_port.write(RECEIVE_STOP_PAGE);

            //  Clear ISR
            ne2000.registers.isr_port.lock().write(0xFF);

            // Initialize IMR
            ne2000.registers.imr_port.lock().write(
                (InterruptMaskRegister::IMR_PRXE
                    | InterruptMaskRegister::IMR_PTXE
                    | InterruptMaskRegister::IMR_OVWE)
                    .bits(),
            );

            // Switch to P1, disable DMA and Stop the NIC */
            ne2000
                .registers
                .command_port
                .write((CR::STP | CR::STOP_DMA | CR::PAGE_1).bits());

            // Initialize Physical Address Register: PAR0-PAR5
            //each mac address bit is written two times into the buffer

            //let mut packet = [0u8; 40];

            //for byte in packet.iter_mut() {
            //    *byte = ne2000.registers.data_port.read();
            //}

            //for byte in packet.iter_mut() {
            //    info!("content: 0x{:02X} ", byte);
            //}

            /*

            self.registers.par_0.write(mac[0]);
            self.registers.par_1.write(mac[1]);
            self.registers.par_2.write(mac[2]);
            self.registers.par_3.write(mac[3]);
            self.registers.par_4.write(mac[4]);
            self.registers.par_5.write(mac[5]);*/
            let mut mac = [0u8; 6];

            // define the location of the data for the mac address
            let mut par_ports: [Port<u8>; 6] = [
                Port::new(ne2000.base_address + 0x01),
                Port::new(ne2000.base_address + 0x02),
                Port::new(ne2000.base_address + 0x03),
                Port::new(ne2000.base_address + 0x04),
                Port::new(ne2000.base_address + 0x05),
                Port::new(ne2000.base_address + 0x06),
            ];
            // iterate through the ports to get the mac address
            for (i, port) in par_ports.iter_mut().enumerate() {
                mac[i] = port.read();
            }

            // Print buffer contents (just for debugging)
            // TODO: remove probably at the end
            for (i, byte) in mac.iter().enumerate() {
                info!("buffer[{:02}] = 0x{:02X}", i, byte);
            }

            // Switch to Page 1 to access PAR0..PAR5
            //ne2000
            //    .registers
            //    .command_port
            //    .write((CR::PAGE_1 | CR::STOP_DMA | CR::STP).bits());

            // Write MAC address to PAR registers (every second byte)
            ne2000.registers.par_0.write(mac[0]);
            ne2000.registers.par_1.write(mac[1]);
            ne2000.registers.par_2.write(mac[2]);
            ne2000.registers.par_3.write(mac[3]);
            ne2000.registers.par_4.write(mac[4]);
            ne2000.registers.par_5.write(mac[5]);

            //TODO: just for testing remove at end
            info!(
                "NE2000 MAC address: [{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}]",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
            );

            // located on Page 1
            // Initialize Multicast Address Register: MAR0-MAR7 with 0xFF
            // TODO: add reference handbook
            ne2000.registers.mar0.write(0xFF);
            ne2000.registers.mar1.write(0xFF);
            ne2000.registers.mar2.write(0xFF);
            ne2000.registers.mar3.write(0xFF);
            ne2000.registers.mar4.write(0xFF);
            ne2000.registers.mar5.write(0xFF);
            ne2000.registers.mar6.write(0xFF);
            ne2000.registers.mar7.write(0xFF);

            // P.156 http://www.bitsavers.org/components/national/_dataBooks/1988_National_Data_Communications_Local_Area_Networks_UARTs_Handbook.pdf#page=156
            CURRENT_NEXT_PAGE_POINTER = RECEIVE_START_PAGE + 1;

            // Initialize Current Pointer: CURR
            ne2000.registers.curr.write(CURRENT_NEXT_PAGE_POINTER);

            // 10) Start NIC
            ne2000
                .registers
                .command_port
                .write((CR::STOP_DMA | CR::STA | CR::PAGE_0).bits());

            //Initialize TCR and RCR
            ne2000.registers.tcr_port.write(0);
            ne2000.registers.rcr_port.write(
                (ReceiveConfigurationRegister::RCR_AR
                    | ReceiveConfigurationRegister::RCR_AB
                    | ReceiveConfigurationRegister::RCR_AM)
                    .bits(),
            );

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
            //let cr: u8 = unsafe { command_port.read() };

            info!("\x1b[1;31mFinished Initialization");
            // print an ascii banner to the log screen
            info!(include_str!("banner.txt"), ne2000.read_mac(), base_address);
            scheduler().sleep(1000);
        }
        // set the static once
        let ptr = &mut ne2000 as *mut Ne2000;
        NE2000_PTR.store(ptr, Ordering::SeqCst);

        /*scheduler().ready(Thread::new_kernel_thread(
            Ne2000::ne2000_interrupt_thread,
            "NE2000 Interrupts",
        ));*/

        ne2000
    }

    // TODO: check how to build a correct data packet in the documentation

    pub fn send_packet(&mut self, packet: &[u8]) {
        let packet_length = packet.len() as u16;

        unsafe {
            // check, if the nic is ready for transmit
            while CR::from_bits_retain(self.registers.command_port.read()).contains(CR::TXP) {
                scheduler().sleep(1);
                info!("Transmit bit still set!");
            }

            //dummy_read
            //TODO: (see thiel bachelor thesis), add reference from handbook

            info!("Start Dummy Read");

            // switch to page 0, enable nic, stop dma
            self.registers
                .command_port
                .write((CR::STA | CR::STOP_DMA | CR::PAGE_0).bits());

            // Save CRDA bit
            let old_crda: u16 = self.registers.crda0_p0.read() as u16
                | ((self.registers.crda1_p0.read() as u16) << 8);

            // Set RBCR > 0
            self.registers.rbcr0.write(0x01);
            self.registers.rbcr1.write(0x00);
            // Set RSAR to unused address
            self.registers.rsar0.write(TRANSMIT_START_PAGE);
            self.registers.rsar1.write(0);
            // Issue Dummy Remote READ Command
            self.registers
                .command_port
                .write((CR::STA | CR::REMOTE_READ | CR::PAGE_0).bits());

            // Mandatory Delay between Dummy Read and Write to ensure dummy read was successful
            // Wait until crda value has changed
            while old_crda
                == self.registers.crda0_p0.read() as u16
                    | ((self.registers.crda1_p0.read() as u16) << 8)
            {
                scheduler().sleep(1);
                info!("not equal")
            }
            info!("Finished Dummy Read");

            // end dummy read

            info!("Load packet size and enable remote write");
            // Load RBCR with packet size
            let low = (packet_length & 0xFF) as u8;
            let high = (packet_length >> 8) as u8;
            self.registers.rbcr0.write(low);
            self.registers.rbcr1.write(high);
            // Clear RDC Interrupt
            self.registers
                .isr_port
                .lock()
                .write(InterruptStatusRegister::ISR_RDC.bits());
            // Load RSAR with 0 (low bits) and Page Number (high bits)
            self.registers.rsar0.write(0);
            self.registers.rsar1.write(TRANSMIT_START_PAGE);
            // Set COMMAND to remote write
            self.registers
                .command_port
                .write((CR::STA | CR::REMOTE_WRITE | CR::PAGE_0).bits());

            // Write packet to remote DMA
            let data_port = &mut self.registers.data_port;

            for &data in packet {
                data_port.write(data);
            }

            // Poll ISR until remote DMA Bit is set
            while (self.registers.read_isr() & InterruptStatusRegister::ISR_RDC.bits()) == 0 {
                scheduler().sleep(1);
                info!("polling")
            }

            // Clear ISR RDC Interrupt
            self.registers
                .isr_port
                .lock()
                .write(InterruptStatusRegister::ISR_RDC.bits());

            // Set TBCR Bits before Transmit and TPSR Bit
            self.registers.tbcr0_p0.write(low);
            self.registers.tbcr1_p0.write(high);
            self.registers.tpsr.write(TRANSMIT_START_PAGE);

            // Set TXP Bit to send packet
            self.registers
                .command_port
                .write((CR::STA | CR::TXP | CR::STOP_DMA | CR::PAGE_0).bits());

            info!("finished send_packet fn");
        }
    }

    pub fn receive_packet(&mut self) {
        unsafe {
            // Read current register to prepare for the next packet
            // switch to page 1 to read curr register
            // switch back to Page 0
            self.registers
                .command_port
                .write((CR::STA | CR::STOP_DMA | CR::PAGE_1).bits());

            let current = self.registers.curr.read();
            self.registers
                .command_port
                .write((CR::STA | CR::STOP_DMA | CR::PAGE_0).bits());

            // as long as packets are there to be processed, loop
            while current != CURRENT_NEXT_PAGE_POINTER {
                // write size of header
                self.registers
                    .rbcr0
                    .write(mem::size_of::<PacketHeader>() as u8);
                self.registers.rbcr1.write(0);
                self.registers.rsar0.write(0);
                self.registers.rsar1.write(CURRENT_NEXT_PAGE_POINTER);

                // enable remote Read
                self.registers
                    .command_port
                    .write((CR::STA | CR::REMOTE_READ | CR::PAGE_0).bits());

                // build the PacketHeader struct from the buffer ring
                // the nic always stores a packet header at the beginning of the first
                // buffer page which is used to store the received package
                // the nic itself attaches the a 4 Byte header to each packet
                // TODO: add reference
                let packet_header = PacketHeader {
                    receive_status: self.registers.data_port.read() as u8,
                    next_packet: self.registers.data_port.read() as u8,
                    length: {
                        // Read the first byte (u8)
                        let low_byte = self.registers.data_port.read() as u16;

                        // Read the second byte (u8), shift it by 8 bits to form the higher part of the length
                        let high_byte = self.registers.data_port.read() as u16;

                        // Combine the two bytes to form the full length (in u16)
                        let length_u16 = (high_byte << 8) | low_byte;

                        // Subtract the size of the packet header
                        let length_without_header = length_u16 - size_of::<PacketHeader>() as u16;

                        // Return the length as u8
                        length_without_header as u16
                    },
                };

                info!("packet header rcr : {}", packet_header.receive_status);
                info!("packet header length : {}", packet_header.length);
                info!("packet header next_packet: {}", packet_header.next_packet);

                // check received packet

                // rust doesn't treat integers as boolean in an if clause, so a comparison has to be made
                // TODO: What does 1 mean for receive_status ?
                if (packet_header.receive_status & ReceiveStatusRegister::RSR_PRX.bits()) != 0
                    && packet_header.length as u32 <= MAXIMUM_ETHERNET_PACKET_SIZE as u32
                {
                    // get an empty packet from the receive_buffers_empty queue for
                    // saving the data
                    // 0 is the Receiver
                    let mut packet = self
                        .receive_buffers_empty
                        .0
                        .try_dequeue()
                        .expect("Error dequeuing");
                    // Write packet length into RBCR
                    self.registers
                        .rbcr0
                        .write(packet_header.length as u8 & 0xFF);
                    //self.registers.rbcr1.write(packet_header.length >> 8);
                    // fix overflow warning
                    let length: u16 = packet_header.length as u16;
                    self.registers.rbcr1.write((length >> 8) as u8);
                    // Set RSAR0 to nicHeaderLength to skip the packet header during the read operation
                    self.registers.rsar0.write(size_of::<PacketHeader>() as u8);
                    self.registers.rsar1.write(CURRENT_NEXT_PAGE_POINTER);
                    // issue remote read operation for reading the packet from the nics local buffer
                    self.registers
                        .command_port
                        .write((CR::STA | CR::REMOTE_READ | CR::PAGE_0).bits());

                    // Read Packet Data from I/O Port and write it into packet
                    //self.registers.data_port.read() as u8;
                    for i in 0..packet_header.length {
                        // slice indices must be of type usize
                        packet[i as usize] = self.registers.data_port.read() as u8;
                    }
                    // enqueue the packet in the receive_messages queue, this queue gets processed by
                    // receive in smoltcp
                    self.receive_messages
                        .1
                        .try_enqueue(packet)
                        .expect("Error enqueuing packet");
                }
                // update pointers for the next package
                CURRENT_NEXT_PAGE_POINTER = packet_header.next_packet;
                if (packet_header.next_packet - 1) < RECEIVE_START_PAGE {
                    self.registers.bnry_port.write(RECEIVE_STOP_PAGE - 1);
                } else {
                    self.registers
                        .bnry_port
                        .write(CURRENT_NEXT_PAGE_POINTER - 1);
                }
            }
            // clear the RDC Interrupt (Remote DMA Operation is complete)
            self.registers
                .isr_port
                .lock()
                .write(InterruptStatusRegister::ISR_RDC.bits());
        }
    }

    // read the mac address and return it
    // the mac is needed for checking if received packets
    // are addressed to the nic
    pub fn read_mac(&self) -> EthernetAddress {
        let mut mac2 = [0u8; 6];
        let mut par_registers = self.par_registers.id.lock();

        unsafe {
            //Read 6 bytes (MAC address)

            // switch to page 1 to access PAR 0..5
            //self.registers.command_port.write(0x40);
            let mut registers = Registers::new(self.base_address);
            registers.command_port.write(0x40);

            mac2[0] = par_registers.0.read();
            mac2[1] = par_registers.1.read();
            mac2[2] = par_registers.2.read();
            mac2[3] = par_registers.3.read();
            mac2[4] = par_registers.4.read();
            mac2[5] = par_registers.5.read();

            registers
                .command_port
                .write((CR::STOP_DMA | CR::STA | CR::PAGE_0).bits());

            // check if on correct Page (on Page 1 are the PARs Registers for the MAC Adress)

            /*let mut command_port = Port::<u8>::new(self.base_address + 0x00);
            let cr = command_port.read();
            let ps = (cr >> 6) & 0b11;

            match ps {
                0 => info!("Currently on Page 0"),
                1 => info!("Currently on Page 1"),
                2 => info!("Currently on Page 2"),
                3 => info!("Currently on Page 3"),
                _ => unreachable!(),
            }*/
        }
        // convert the data in the array to type EthernetAddress
        let mac_address = EthernetAddress::from_bytes(&mac2);
        // return the actual MAC Address
        mac_address
    }

    // gets called, if the buffer ring is full
    // this is analogous to the nic datasheet
    // TODO: add reference
    pub fn handle_overflow_interrupt(&mut self) {
        unsafe {
            // 1. save the value of the TXP Bit in CR
            let txp_bit = self.registers.command_port.read() & CR::TXP.bits();
            // 2. Issue stop command
            self.registers
                .command_port
                .write((CR::STOP | CR::PAGE_0).bits());

            // 3. wait for at least 1.6 ms according to the documentation
            // TODO: add reference
            scheduler().sleep(1600);
            // 4. Clear RBCR0 and RBCR1
            self.registers.rbcr0.write(0);
            self.registers.rbcr1.write(0);
            // 5. read value of TXP bit, check if there was a transmission in progress when the
            // stop command was issued
            // if value = 0 -> set resend = 0
            // if value = 1 -> read ISR
            //      if PTX or TXE = 1 -> resend = 0
            //      else resend 1
            let mut resend = 0;
            if txp_bit == 0 {
                resend = 0;
            }
            if txp_bit == 1 {
                if self.registers.read_isr()
                    & (InterruptStatusRegister::ISR_PTX | InterruptStatusRegister::ISR_TXE).bits()
                    != 0
                {
                    resend = 0
                } else {
                    resend = 1;
                }
            }

            // 6. Place the nic in loopback mode 0
            self.registers
                .tcr_port
                .write(TransmitConfigurationRegister::TCR_LB0.bits());
            // 7. Issue start command
            self.registers
                .command_port
                .write((CR::STA | CR::PAGE_0).bits());
            // 8. remove packets in the buffer
            self.receive_packet();
            //9. Reset Overwrite warning (OVW)
            self.registers
                .isr_port
                .lock()
                .write(InterruptStatusRegister::ISR_OVW.bits());
            //10. take nic out of loopback
            self.registers.tcr_port.write(0);

            //11. if resend = 1, reset variable, reissue transmit command
            if resend == 1 {
                self.registers
                    .command_port
                    .write((CR::STA | CR::TXP | CR::STOP_DMA | CR::PAGE_0).bits());
            }
        }
        let mut ovw = self.interrupts.ovw.lock();
        *ovw = false;
    }

    // assign driver to interrupt handler
    pub fn assign(device: Arc<Ne2000>) {
        let interrupt = device.interrupt;
        interrupt_dispatcher().assign(interrupt, Box::new(Ne2000InterruptHandler::new(device)));
        apic().allow(interrupt);
    }
}

// the interrupt handler holds a shared reference to the Ne2000 device
pub struct Ne2000InterruptHandler {
    device: Arc<Ne2000>,
}

// implement the InterruptHandler
// creates a new Instance of Ne2000InterruptHandler
impl Ne2000InterruptHandler {
    pub fn new(device: Arc<Ne2000>) -> Self {
        Self { device }
    }
}

// gets called, if the nic receives or transmits a package
// or if an Buffer Overflow occurs
impl InterruptHandler for Ne2000InterruptHandler {
    fn trigger(&self) {
        // a mutex is required for IMR and ISR, because these registers are also used by the
        // transmit and receive function, and Init routine
        if self.device.registers.isr_port.is_locked() {
            panic!("Interrupt status register is locked during interrupt!");
        }

        // clear Interrupt Mask Register
        self.device.registers.write_imr(0);

        // Read interrupt status register (Each bit corresponds to an interrupt type or error)
        let status_reg = self.device.registers.read_isr();
        info!("Hello from trigger");
        let status = InterruptStatusRegister::from_bits_retain(status_reg);

        // Check interrupt flags
        // Packet Reception Flag set (PRX) ? (Packet received?)
        if status.contains(InterruptStatusRegister::ISR_PRX) {
            info!("Packet received");
            // reset prx bit in isr
            unsafe {
                self.device
                    .registers
                    .isr_port
                    .lock()
                    .write(InterruptStatusRegister::ISR_PRX.bits());
            };
            self.device.rcv.store(true, Ordering::Relaxed);

            // lock rcv Variable
            // dereference the MutexGuard to access the value
            // call the packet received method
        }

        // check for Packet Transmission Interrupt
        if status.contains(InterruptStatusRegister::ISR_PTX) {
            info!("Packet transmission");
            //self.device.interrupts.ovw = true;
            // reset ptx bit in isr
            unsafe {
                self.device
                    .registers
                    .isr_port
                    .lock()
                    .write(InterruptStatusRegister::ISR_PTX.bits());
            }
            // free the allocated memory after sending the packet
            let mut queue = self.device.send_queue.0.lock();
            let mut buffer = queue.try_dequeue();
            while buffer.is_ok() {
                unsafe { frames::free(buffer.unwrap()) };
                buffer = queue.try_dequeue();
            }
        }
        // check for an buffer overflow
        if status.contains(InterruptStatusRegister::ISR_OVW) {
            // `self.device` is of type `Arc<Ne2000>`, which is the shared reference
            let device_ref: &Ne2000 = &self.device; // This is a shared reference
            // Use unsafe to get a mutable reference to the inner `Ne2000` object
            let device_mut = unsafe {
                // Convert from a shared reference to a mutable raw pointer
                ptr::from_ref(device_ref)
                    .cast_mut() // Cast to a mutable pointer
                    .as_mut() // Convert the raw pointer back to a mutable reference
                    .unwrap() // Unwrap to ensure it’s valid
            };
            // call the method
            device_mut.handle_overflow_interrupt();
            let mut ovw = self.device.interrupts.ovw.lock();
            *ovw = true;
        }
    }
}

// Tests, not working because of std remove at the end
#[cfg(test)]
mod tests {
    use super::CR;

    #[test]
    fn test_command_register_bits() {
        // STA | TXP | PAGE_0
        let expected: u8 = 0b00000110; // STA = 0x02, TXP = 0x04
        let combined = CR::STA | CR::TXP | CR::PAGE_0;

        assert_eq!(combined.bits(), expected, "Combined CR bits are incorrect");
    }
}
