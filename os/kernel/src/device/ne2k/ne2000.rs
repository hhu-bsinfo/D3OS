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
use crate::memory::{PAGE_SIZE, frames};
use crate::{apic, device, interrupt_dispatcher, pci_bus, process_manager, scheduler};
use core::mem;
use core::ops::BitOr;
use core::{ptr, slice};
use log::info;
// for allocator impl
use core::alloc::{AllocError, Allocator, Layout};
// for allocator impl
use crate::interrupt::interrupt_handler::InterruptHandler;
use core::ptr::NonNull;
use spin::{Mutex, RwLock};

// lock free algorithms and datastructes
// queues: different queue implementations
// mpsc : has the jiffy queue ; lock-free unbounded, for send
// mpmpc : multiple producers, multiple consumers, for receive
use nolock::queues::{mpmc, mpsc};

use pci_types::{CommandRegister, EndpointHeader};
// smoltcp provides a full network stack for creating packets, sending, receiving etc.
use alloc::sync::Arc;
use alloc::vec::Vec;
use smoltcp::phy;
use smoltcp::phy::{DeviceCapabilities, Medium};
use smoltcp::time::Instant;
use smoltcp::wire::EthernetAddress;

// for writing to the registers
use alloc::str;
use alloc::string::String;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};
use x86_64::{PhysAddr, VirtAddr};

// super looks in a relative path for other modules
use super::register_flags::{
    CR, DataConfigurationRegister, InterruptMaskRegister, InterruptStatusRegister,
    ReceiveConfigurationRegister, TransmitConfigurationRegister,
};

use super::network_stack::*;

// =============================================================================

//type Ne2000Device = Arc<Mutex<Ne2000>>;
const RECV_QUEUE_CAP: usize = 16;
static RESET: u8 = 0x1F;
static TRANSMIT_START_PAGE: u8 = 0x40;
const DISPLAY_RED: &'static str = "\x1b[1;31m";
static MINIMUM_ETHERNET_PACKET_SIZE: u8 = 64;
static MAXIMUM_ETHERNET_PACKET_SIZE: u32 = 1522;
static mut CURRENT_NEXT_PAGE_POINTER: u8 = 0x00;

// Reception Buffer Ring Start Page
// http://www.osdever.net/documents/WritingDriversForTheDP8390.pdf
// Page 4 PSTART
static RECEIVE_START_PAGE: u8 = 0x46;

//Reception Buffer Ring End
//P.4 PSTOP http://www.osdever.net/documents/WritingDriversForTheDP8390.pdf
static RECEIVE_STOP_PAGE: u8 = 0x80;

// 0x80 - 0x46 = 0x58 = 58 pages
// total buffer size = 58 * 256 Bytes  = 14.KiB

// The Structure of the PacketHeader is definied in the datasheet
// TODO: add reference
// receive status : holds the content of the Receive Status Register
// next_packet : Pointer, which holds the next ringbuffer address

#[repr(C)]
struct PacketHeader {
    receive_status: u8,
    next_packet: u8,
    length: u8,
}

struct ParRegisters {
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
    fn new(base_address: u16) -> Self {
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
    data_port: Port<u16>,
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
    fn new(base_address: u16) -> Self {
        // TODO: replace hex with Register names defined in a different struct for better readibility
        Self {
            reset_port: Port::new(base_address + 0x1F),
            command_port: Port::new(base_address + 0x00),
            rsar0: Port::new(base_address + 0x08),
            rsar1: Port::new(base_address + 0x09),
            rbcr0: Port::new(base_address + 0x0A),
            rbcr1: Port::new(base_address + 0x0B),
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
    pub send_queue: (
        Mutex<mpsc::jiffy::Receiver<PhysFrameRange>>,
        mpsc::jiffy::Sender<PhysFrameRange>,
    ),
    receive_buffer: Mutex<ReceiveBuffer>,
    pub receive_buffers_empty: (
        mpmc::bounded::scq::Receiver<Vec<u8, PacketAllocator>>,
        // Sender send data to a set of Receivers
        mpmc::bounded::scq::Sender<Vec<u8, PacketAllocator>>,
    ),
    receive_messages: (
        mpmc::bounded::scq::Receiver<Vec<u8, PacketAllocator>>,
        mpmc::bounded::scq::Sender<Vec<u8, PacketAllocator>>,
    ),
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
        info!("NE2000 base address: [0x{:x}]", base_address);

        // mpsc (multiple-producer, single-consumer)
        // FIFO queue implementation

        // queue creates a new empty queue and returns (Receiver, Sender)
        // send_queue.enqueue(13) -> enque data
        // see: https://docs.rs/nolock/latest/nolock/queues/mpsc/jiffy/index.html
        let send_queue = mpsc::jiffy::queue();
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

        let mut ne2000 = Self {
            registers: Registers::new(base_address),
            base_address: base_address,
            par_registers: ParRegisters::new(base_address),
            send_queue: (Mutex::new(send_queue.0), send_queue.1),
            receive_buffers_empty: recv_buffers,
            receive_buffer: Mutex::new(ReceiveBuffer::new()),
            receive_messages: mpmc::bounded::scq::queue(RECV_QUEUE_CAP),
        };

        //ne2000.init();
        //let mut buffer = [0u8; 1514];
        //let data = &mut buffer[..1514];
        //ne2000.send_packet(data);

        //}

        //pub fn init(&mut self) {
        info!("\x1b[1;31mPowering on device");
        info!(include_str!("banner.txt"), " ", base_address);
        scheduler().sleep(1500);
        unsafe {
            //command_port.write(0x02);
            //let registers = &mut self.registers;
            //let j = self.registers.isr_port.read();
            //info!("ISR: {}", j);
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
            let a = ne2000.registers.reset_port.read();
            ne2000.registers.reset_port.write(a);
            //info!("1: 0x{:X}", reset_port_value);
            //reset_port.write(a);
            //let isr_value = ne2000.registers.isr_port.read();
            //info!("ISR: 0x{:X}", isr_value);
            //self.registers.isr_port.write(isr_value);

            // bitwise and operation, checks if highest bit is set
            while (ne2000.registers.isr_port.lock().read() & 0x80) == 0 {
                info!("Reset in Progress");
            }
            info!("\x1b[1;31mNe2000 reset complete");

            info!("\x1b[1;31mInitializing Registers of Device Ne2000");

            // Initialize CR Register
            ne2000
                .registers
                .command_port
                .write((CR::STOP_DMA | CR::STP | CR::PAGE_0).bits());
            //info!("cr: {}", ne2000.registers.command_port.read());
            //scheduler().sleep(100);

            // Initialize DCR Register
            //info!(
            //    "DCR after setting bits: {:#x}",
            //    (DataConfigurationRegister::DCR_AR
            //        | DataConfigurationRegister::DCR_FT1
            //        | DataConfigurationRegister::DCR_LS)
            //        .bits()
            //);

            // Command Register at Page 0 at this point
            ne2000.registers.dcr_port.write(
                (DataConfigurationRegister::DCR_AR
                    | DataConfigurationRegister::DCR_FT1
                    | DataConfigurationRegister::DCR_LS)
                    .bits(),
            );
            //ne2000.registers.command_port.write((CR::PAGE_2).bits());
            //info!("dcr: {}", ne2000.registers.dcr_port.read());

            // clear RBCR1,0
            //RBCR0,1 : indicates the length of the block in bytes
            // MAC address has length of 6 Bytes
            ne2000.registers.rbcr0.write(0);
            ne2000.registers.rbcr1.write(0);
            //info!("rbcr0: {}", self.registers.rbcr0.read());
            //info!("rbcr1: {}", self.registers.rbcr1.read());

            // initialize RCR
            ne2000.registers.rcr_port.write(
                (ReceiveConfigurationRegister::RCR_AR
                    | ReceiveConfigurationRegister::RCR_AB
                    | ReceiveConfigurationRegister::RCR_AM)
                    .bits(),
            );

            // Place the NIC in Loopback Mode (Mode 1)
            ne2000
                .registers
                .tcr_port
                .write(TransmitConfigurationRegister::TCR_LB0.bits());

            // initialize buffer
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

            //Read 6 bytes (MAC address)
            /*for byte in mac.iter_mut() {
                *byte = self.registers.data_port.read();
            }

            self.registers.par_0.write(mac[0]);
            self.registers.par_1.write(mac[1]);
            self.registers.par_2.write(mac[2]);
            self.registers.par_3.write(mac[3]);
            self.registers.par_4.write(mac[4]);
            self.registers.par_5.write(mac[5]);*/

            //ne2000
            //    .registers
            //    .command_port
            //    .write((CR::PAGE_1 | CR::RD_1 | CR::STA).bits());

            let mut mac = [0u8; 6];

            let mut par_ports: [Port<u8>; 6] = [
                Port::new(ne2000.base_address + 0x01),
                Port::new(ne2000.base_address + 0x02),
                Port::new(ne2000.base_address + 0x03),
                Port::new(ne2000.base_address + 0x04),
                Port::new(ne2000.base_address + 0x05),
                Port::new(ne2000.base_address + 0x06),
            ];
            for (i, port) in par_ports.iter_mut().enumerate() {
                //mac[i] = port.read();
                mac[i] = port.read();
            }

            // Print buffer contents
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

            info!(
                "NE2000 MAC address: [{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}]",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
            );

            // Optionally switch back to Page 0
            //ne2000
            //    .registers
            //    .command_port
            //    .write((CR::PAGE_0 | CR::STOP_DMA | CR::STP).bits());

            //let mut command_port = Port::<u8>::new(ne2000.base_address + 0x00);
            //let cr = command_port.read();
            //let ps = (cr >> 6) & 0b11;

            /*match ps {
                0 => info!("Currently on Page 0"),
                1 => info!("Currently on Page 1"),
                2 => info!("Currently on Page 2"),
                3 => info!("Currently on Page 3"),
                _ => unreachable!(),
            }*/

            // located on Page 1
            /* 9) ii) Initialize Multicast Address Register: MAR0-MAR7 with 0xFF */
            ne2000.registers.mar0.write(0xFF);
            ne2000.registers.mar1.write(0xFF);
            ne2000.registers.mar2.write(0xFF);
            ne2000.registers.mar3.write(0xFF);
            ne2000.registers.mar4.write(0xFF);
            ne2000.registers.mar5.write(0xFF);
            ne2000.registers.mar6.write(0xFF);
            ne2000.registers.mar7.write(0xFF);

            /* P.156 http://www.bitsavers.org/components/national/_dataBooks/1988_National_Data_Communications_Local_Area_Networks_UARTs_Handbook.pdf#page=156
            Accessed: 2024-03-29
            */
            CURRENT_NEXT_PAGE_POINTER = RECEIVE_START_PAGE + 1;

            /* 9) iii) Initialize Current Pointer: CURR */
            ne2000.registers.curr.write(CURRENT_NEXT_PAGE_POINTER);

            /* 10) Start NIC */
            ne2000
                .registers
                .command_port
                .write((CR::STOP_DMA | CR::STA | CR::PAGE_0).bits());
            //ne2000.registers.command_port.write(0x22);
            /*info!(
                "CR REAd after init: {}",
                ne2000.registers.command_port.read()
            );*/

            /* 11) Initialize TCR and RCR */
            ne2000.registers.tcr_port.write(0);
            ne2000.registers.rcr_port.write(
                (ReceiveConfigurationRegister::RCR_AR
                    | ReceiveConfigurationRegister::RCR_AB
                    | ReceiveConfigurationRegister::RCR_AM)
                    .bits(),
            );
            /*info!(
                "CR REAd after init: {}",
                ne2000.registers.command_port.read()
            );*/

            //Set up Remote DMA to read from address 0x0000
            // RSAR0,1 : points to the start of the block of data to be transfered
            //info!("rb0: {}", rbcr0.read());
            //info!("rb1: {}", rbcr1.read());

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

            /*unsafe {
                let mac = [
                    self.registers.par_0.read(),
                    self.registers.par_1.read(),
                    self.registers.par_2.read(),
                    self.registers.par_3.read(),
                    self.registers.par_4.read(),
                    self.registers.par_5.read(),
                ];

                info!("MAC ADRESS INIT: {}", EthernetAddress::from_bytes(&mac));
            }*/
            info!("\x1b[1;31mFinished Initialization");
        }
        //let dummy: [u8; 0] = [];
        //ne2000.send_packet(&dummy);
        ne2000
    }

    // TODO: check how to build a correct data packet in the documentation

    pub fn send_packet(&mut self, packet: &[u8]) {
        let packet_length = packet.len() as u16;
        info!("i hope this works");

        // check, if the nic is ready for transmit
        unsafe {
            //info!("status cr {}", self.registers.command_port.read());
            //let transmit_status = !(self.registers.command_port.read() & CR::TXP.bits());
            //while transmit_status != 0 {
            //    info!("{transmit_status}");
            //}

            info!("i hope this works 2");
            while CR::from_bits_retain(self.registers.command_port.read()).contains(CR::TXP) {
                scheduler().sleep(1);
                info!("Transmit bit still set!");
            }

            //dummy_read (see thiel bachelor thesis)
            info!("Start Dummy Read");

            // switch to page 0, enable nic, stop dma
            self.registers
                .command_port
                .write((CR::STA | CR::STOP_DMA | CR::PAGE_0).bits());

            // 1) Save CRDA bit
            let old_crda: u16 = self.registers.crda0_p0.read() as u16
                | ((self.registers.crda1_p0.read() as u16) << 8);

            // 2.1 ) Set RBCR > 0
            self.registers.rbcr0.write(0x01);
            self.registers.rbcr1.write(0x00);
            // 2.2) Set RSAR to unused address
            self.registers.rsar0.write(TRANSMIT_START_PAGE);
            self.registers.rsar1.write(0);
            // 3) Issue Dummy Remote READ Command
            self.registers
                .command_port
                .write((CR::STA | CR::REMOTE_READ | CR::PAGE_0).bits());

            // 4) Mandatory Delay between Dummy Read and Write to ensure dummy read was successful
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
            // 1) Load RBCR with packet size
            let low = (packet_length & 0xFF) as u8;
            let high = (packet_length >> 8) as u8;
            self.registers.rbcr0.write(low);
            self.registers.rbcr1.write(high);
            // 2) Clear RDC Interrupt
            self.registers
                .isr_port
                .lock()
                .write(InterruptStatusRegister::ISR_RDC.bits());
            // 3) Load RSAR with 0 (low bits) and Page Number (high bits)
            self.registers.rsar0.write(0);
            self.registers.rsar1.write(TRANSMIT_START_PAGE);
            // 4) Set COMMAND to remote write
            self.registers
                .command_port
                .write((CR::STA | CR::REMOTE_WRITE | CR::PAGE_0).bits());

            // 5) Write packet to remote DMA
            let data_port = &mut self.registers.data_port;

            for &data in packet {
                data_port.write(data as u16);
            }

            // 6) Poll ISR until remote DMA Bit is set
            while (self.registers.isr_port.lock().read() & InterruptStatusRegister::ISR_RDC.bits())
                == 0
            {
                scheduler().sleep(1);
                info!("polling")
            }

            // 7) Clear ISR RDC Interrupt
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
                let packet_header = PacketHeader {
                    receive_status: self.registers.data_port.read() as u8,
                    next_packet: self.registers.data_port.read() as u8,
                    length: self.registers.data_port.read() as u8
                        + (((self.registers.data_port.read() as u16) << 8)
                            - size_of::<PacketHeader>() as u16) as u8,
                };

                // check received packet

                // rust doesn't treat integers as boolean in an if clause, so a comparison has to be made
                if (packet_header.receive_status & ReceiveStatusRegister::RSR_PRX.bits()) != 0
                    && packet_header.length as u32 <= MAXIMUM_ETHERNET_PACKET_SIZE as u32
                {
                    let mut packet: [u8; 1522] = [0u8; 1522];
                    // Write packet length into RBCR
                    self.registers.rbcr0.write(packet_header.length & 0xFF);
                    //self.registers.rbcr1.write(packet_header.length >> 8);
                    // fix overflow warning
                    let length: u16 = packet_header.length as u16;
                    self.registers.rbcr1.write((length >> 8) as u8);
                    // Set RSAR0 to nicHeaderLength to skip the packet header during the read operation
                    self.registers.rsar0.write(size_of::<PacketHeader>() as u8);
                    self.registers.rsar1.write(CURRENT_NEXT_PAGE_POINTER);
                    self.registers
                        .command_port
                        .write((CR::STA | CR::REMOTE_READ | CR::PAGE_0).bits());

                    // Read Packet Data from I/O Port and write it into packet */
                    for i in 0..packet_header.length {
                        // slice indices must be of type usize
                        packet[i as usize] = self.registers.data_port.read() as u8;
                    }
                    // let smoltcp handle the packet
                    // TODO: check network/mod.rs for the handling of the packet
                    // probably an interrupt handler has to be assigned, check this
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
            self.registers
                .isr_port
                .lock()
                .write(InterruptStatusRegister::ISR_RDC.bits());
        }
    }

    // read the mac address and return it as array
    //pub fn read_mac(&mut self) -> [u8; 6] {
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

            //self.registers
            //    .command_port
            //    .write((CR::STOP_DMA | CR::STA | CR::PAGE_0).bits());
            registers
                .command_port
                .write((CR::STOP_DMA | CR::STA | CR::PAGE_0).bits());
            //info!("CR REAd after init: {}", registers.command_port.read());

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
        let address3 = EthernetAddress::from_bytes(&mac2);
        //info!("fn read_mac: ({})", address3);
        //mac2
        address3
    }
}

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

impl InterruptHandler for Ne2000InterruptHandler {
    fn trigger(&self) {
        // a mutex is required for IMR and ISR, because these registers are also used by the
        // transmit and receive function, and Init routine
        if self.device.registers.isr_port.is_locked() {
            panic!("Interrupt status register is locked during interrupt!");
        }

        unsafe {
            // clear Interrupt Mask Register
            // add mutex because Arc object,
            self.device.registers.imr_port.lock().write(0);

            // Read interrupt status register (Each bit corresponds to an interrupt type or error)
            let mut status_reg = self.device.registers.isr_port.lock();
            let status = InterruptStatusRegister::from_bits_retain(status_reg.read());

            // Check interrupt flags
            // Packet Reception Flag set (PRX) ?
            if status.contains(InterruptStatusRegister::ISR_PRX) {
                // reset prx bit in isr
                self.device
                    .registers
                    .isr_port
                    .lock()
                    .write(InterruptStatusRegister::ISR_PRX.bits());
                // check for Packet Transmission Interrupt
            } else if status.contains(InterruptStatusRegister::ISR_PTX) {
                // reset ptx bit in isr
                self.device
                    .registers
                    .isr_port
                    .lock()
                    .write(InterruptStatusRegister::ISR_PTX.bits());
            }
            // TODO: write overwrite Method
        }
    }
}

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
