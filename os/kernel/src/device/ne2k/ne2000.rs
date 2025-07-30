// =============================================================================
// FILE        : ne2000.rs
// AUTHOR      : Johann Spenrath <johann.spenrath@hhu.de>
// DESCRIPTION : Main file for the NE2000 driver
// =============================================================================
// NOTES:
// TODO : check trigger method in ne2000.cpp
// rewrite overwrite and receive method, replace self with reg mentioned above
// do the same for the cpp implementation
// =============================================================================
// DEPENDENCIES:
// =============================================================================
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::memory::{PAGE_SIZE, frames};
use crate::process::thread::Thread;
use crate::{apic, interrupt_dispatcher, pci_bus, process_manager, scheduler};
use core::mem;
// for calling the methods outside the interrupt handler
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use core::{ptr, slice};
// print to terminal
use log::info;
// for allocator impl
use alloc::boxed::Box;
use alloc::string::String;
// import interrupt functionalities
use crate::interrupt::interrupt_handler::InterruptHandler;
use spin::{Mutex, RwLock};

// lock free algorithms and datastructes
// queues: different queue implementations
// mpsc : has the jiffy queue ; lock-free unbounded, for send
// mpmpc : multiple producers, multiple consumers, for receive
use nolock::queues::{mpmc, mpsc};
// 
use pci_types::EndpointHeader;
// smoltcp provides a full network stack for creating packets, sending, receiving etc.
use alloc::sync::Arc;
use alloc::vec::Vec;

// for converting the mac address to type EthernetAddress
use smoltcp::wire::EthernetAddress;

use alloc::str;
use x86_64::VirtAddr;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{Page, PageTableFlags};
// =============================================================================
// Include files in the module ne2k
// =============================================================================
// for writing to the registers
// super looks in a relative path for other modules
// load the bitflags for the register into the module
use super::register_flags::*;
use super::register_flags::page_registers_offsets::*;
// smoltcp configuration
use super::network_stack::*;

// =============================================================================
// ==== CONSTANTS
// =============================================================================

const DISPLAY_RED: &'static str = "\x1b[1;31m";

// Capacity for the receive_queue in the ne2000 struct
const RECV_QUEUE_CAP: usize = 16;


// Define the range of a size for an ethernet packet
static MINIMUM_ETHERNET_PACKET_SIZE: u8 = 64;
static MAXIMUM_ETHERNET_PACKET_SIZE: u32 = 1522;

// this variable points to the next packet to be read
static mut CURRENT_NEXT_PAGE_POINTER: u8 = 0;



// Buffer Start Page for the transmitted pages
static TRANSMIT_START_PAGE: u8 = 0x40;

// Reception Buffer Ring Start Page
// http://www.osdever.net/documents/WritingDriversForTheDP8390.pdf
// Page 4 PSTART
static RECEIVE_START_PAGE: u8 = 0x46;

//Reception Buffer Ring End
//P.4 PSTOP http://www.osdever.net/documents/WritingDriversForTheDP8390.pdf
static RECEIVE_STOP_PAGE: u8 = 0x80;
//static RECEIVE_STOP_PAGE: u8 = 0x50;

// 0x80 - 0x46 = 0x58 = 58 pages
// total buffer size = 58 * 256 Bytes  = 14.KiB

// =============================================================================
// ==== STRUCTS
// =============================================================================


// =============================================================================
// Registers on Page0
// =============================================================================
pub struct Page0 {
    crda_0_p0: Port<u8>,
    crda_1_p0: Port<u8>,
    tbcr_0_port_p0: Port<u8>,
    tbcr_1_port_p0: Port<u8>,
    // buffer configuration
    pstart_port: Port<u8>,
    pstop_port: Port<u8>,
    bnry_port: Port<u8>,
    // add Mutex (05.07.2025)
    dcr_port: Port<u8>,
    tcr_port: Port<u8>,
    rcr_port: Port<u8>,
    tpsr_port: Port<u8>,
    rsar_0_port: Port<u8>,
    rsar_1_port: Port<u8>,
    rbcr_0_port: Port<u8>,
    rbcr_1_port: Port<u8>,
}

// =============================================================================
// Registers on Page1
// =============================================================================
pub struct Page1 {
    //Physical Address Registers, for Reading the MAC Address
    // Reference:
    // section "10.8 PHYSICAL ADDRESS REGISTERS (PAR0-PAR5)", 
    // https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
    par: [Mutex<Port<u8>>; 6],
    mar: [Port<u8>; 8],
    current_port: Port<u8>,
}
// define read + write ports for the registers of the ne2k
pub struct Registers {
    reset_port: Port<u8>,
    command_port: Port<u8>,
    data_port: Port<u8>,
    isr_port: Mutex<Port<u8>>,
    imr_port: Mutex<Port<u8>>,
    page0: Page0,
    page1: Page1,
}

// The Structure of the PacketHeader is definied in the datasheet
// Header is 4 KB
// Reference: p.8, Section "Beginning of Reception", p.11 Section "Storage Format for Received Packets", 
// https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
// receive status : holds the content of the Receive Status Register
// next_packet : Pointer, which holds the next ringbuffer address
// length : length of the received data
#[repr(C)]
struct PacketHeader {
    receive_status: u8,
    next_packet: u8,
    length: u16,
}

// TODO: move method calls in trigger to new and set the variables if the
//       given Interrupt occurs
pub struct Interrupts {
    ovw: AtomicBool,
    rcv: AtomicBool,
}

// the interrupt handler holds a shared reference to the Ne2000 device
// defined in
// TODO: add reference
pub struct Ne2000InterruptHandler {
    device: Arc<Ne2000>,
}

// Struct for the Ne2000 driver
pub struct Ne2000 {
    base_address: u16,
    pub registers: Registers,
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
}

// =============================================================================
// ==== IMPLEMENTATIONS
// =============================================================================

impl Page1 {
    pub fn new(base_address: u16) -> Self {
        Self {
        par: core::array::from_fn(|i| Mutex::new(Port::new(base_address + P1_PAR0 + i as u16))),
        mar: core::array::from_fn(|i| Port::new(base_address + P1_MAR0 + i as u16)),
        current_port: Port::new(base_address + P1_CURR),
        }
    }
}

impl Page0 {
    pub fn new(base_address:u16) -> Self {
        Self {
            pstart_port: Port::new(base_address + P0_PSTART ),
            pstop_port: Port::new(base_address + P0_PSTOP),
            bnry_port: Port::new(base_address + P0_BNRY),
            tpsr_port: Port::new(base_address + P0_TPSR),
            tbcr_0_port_p0: Port::new(base_address + P0_TBCR0),
            tbcr_1_port_p0: Port::new(base_address + P0_TBCR1),
            rsar_0_port: Port::new(base_address + P0_RSAR0),
            rsar_1_port: Port::new(base_address + P0_RSAR1),
            rbcr_0_port: Port::new(base_address + P0_RBCR0),
            rbcr_1_port: Port::new(base_address + P0_RBCR1),
            rcr_port: Port::new(base_address + P0_RCR),
            tcr_port: Port::new(base_address + P0_TCR),
            dcr_port: Port::new(base_address + P0_DCR),
            crda_0_p0: Port::new(base_address + P0_CRDA0),
            crda_1_p0: Port::new(base_address + P0_CRDA1),
        }
    }
}
impl Registers {
    pub fn new(base_address: u16) -> Self {
        Self {
            // Adress for reseting the device
            // see: https://wiki.osdev.org/Ne2000#Ne2000_Reset, 
            //      https://wiki.osdev.org/Ne2000#Register_Pages
            reset_port: Port::new(base_address + RESET),
            // command Port for controlling the CR Register
            //(starting, stopping the nic, switching between pages)
            command_port: Port::new(base_address + COMMAND),
            // Interrupt Status Register
            isr_port: Mutex::new(Port::new(base_address + P0_ISR)),
            // Interrupt Mask Register
            imr_port: Mutex::new(Port::new(base_address + P0_IMR)),
            // data port (or i/o port for reading received data)
            data_port: Port::new(base_address + DATA),
            page0: Page0::new(base_address),
            page1: Page1::new(base_address),
            
        }
    }


    fn read_isr(&self) -> u8 {
        unsafe { self.isr_port.lock().read() }
    }
    pub fn write_imr(&self, val: u8) {
        unsafe { self.imr_port.lock().write(val) }
    }
}


// send_queue: needed for packet transmission process in smoltcp
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

impl Ne2000 {
    // =============================================================================
    // ==== FUNCTION new
    // =============================================================================
    // construct new instance of the ne2000 struct and
    // initialize the card and its registers for transmit and receive operations
    // =============================================================================

    // endpoint header contains essential information about the device, 
    // such as the Vendor ID (VID), Device ID (DID), and other configuration parameters
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
        // Reads the IRQ number from the PCI device, 
        // adds the offset and converts into an InterruptVector
        let interrupt =
            InterruptVector::try_from(pci_device.interrupt(pci_config_space).1 + 32).unwrap();
        //reference to the kernel's main process
        let kernel_process = process_manager().read().kernel_process().unwrap();
        // create bounded mpmc queue with RECV_QUEUE_CAP Capacity
        let recv_buffers = mpmc::bounded::scq::queue(RECV_QUEUE_CAP);
        for _ in 0..RECV_QUEUE_CAP {
            // allocate one physical frame 
            let phys_frame = frames::alloc(1);
            
            //map physical frame into the kernel process address space 
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

            // PRESENT: Frame is loaded into memory
            // WRITABLE: Frame is writable
            // NO_CACHE: disable caching
            kernel_process.virtual_address_space.set_flags(
                pages,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE,
            );

            // allocate buffer for physical memory
            let buffer = unsafe {
                Vec::from_raw_parts_in(
                    phys_frame.start.start_address().as_u64() as *mut u8,
                    PAGE_SIZE,
                    PAGE_SIZE,
                    PacketAllocator::default(),
                )
            };
            // enqueue allocated buffer into the receive queue
            recv_buffers
                .1
                .try_enqueue(buffer)
                .expect("Failed to enqueue receive buffer!");
        }

        let interrupts = Interrupts {
            ovw: AtomicBool::new(false),
            rcv: AtomicBool::new(false),
        };

        // construct the ne2000 and return it at the end of the
        // initialization
        let mut ne2000 = Self {
            registers: Registers::new(base_address),
            base_address: base_address,
            send_queue: (Mutex::new(send_queue.0), send_queue.1),
            receive_buffers_empty: recv_buffers,
            receive_buffer: Mutex::new(ReceiveBuffer::new()),
            receive_messages: mpmc::bounded::scq::queue(RECV_QUEUE_CAP),
            interrupt,
            interrupts,
        };

        info!("\x1b[1;31mPowering on device");
        unsafe {
            info!("\x1b[1;31mResetting Device NE2000");

    // =============================================================================
    // ==== INITIALIZATION SEQUENCE FOR THE REGISTERS 
    // =============================================================================
    // Registers get initialized and set as 
    // mentioned on p.29-30, "Section Initialization Sequence"
    // https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
    // =============================================================================

            //=== STEP 0 ===//
            // Reset the NIC
            // Clears the Registers CR, ISR, IMR, DCR, TCR (see NS32490D.pdf, p.29, 11.0 Initialization Procedure)
            // this ensures, that the Registers are cleared and no undefined behavior can happen
            // just doing the read operation enables the reset, a write is not necessary, but the bits dont get set correctly
            // Reference: https://wiki.osdev.org/Ne2000#Initialization_and_MAC_Address
            let reset_value = ne2000.registers.reset_port.read();
            ne2000.registers.reset_port.write(reset_value);

            // bitwise and operation, checks if highest bit is set
            // if register content equals 0, reset was successful
            while (ne2000.registers.read_isr() & 0x80) == 0 {
                info!("Reset in Progress");
                scheduler().sleep(1);
            }

            info!("\x1b[1;31mNe2000 reset complete");
            info!("\x1b[1;31mInitializing Registers of Device Ne2000");

            //=== STEP 1 ===//
            // Initialize CR Register
            // Switch to Page0 , stop DMA and set the NIC in Stop mode
            ne2000
                .registers
                .command_port
                .write((CR::STOP_DMA | CR::STP | CR::PAGE_0).bits());

            //=== STEP 2 ===//
            // Initialize DCR Register
            // Register is used to program the NIC for 8- or 16-bit memory interface,
            // select byte ordering in 16-bit applications and
            // establish FIFO threshholds. The DCR must be initialized prior to loading the Remote Byte Count Registers.
            // Reference: p.22, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
            // Command Register at Page 0 at this point
            ne2000.registers.page0.dcr_port.write(
                (DataConfigurationRegister::DCR_AR
                    | DataConfigurationRegister::DCR_FT1
                    | DataConfigurationRegister::DCR_LS)
                    .bits(),
            );

            //=== STEP 3 ===//
            // clear RBCR1,0
            //RBCR0,1 : indicates the length of the block in bytes
            // MAC address has length of 6 Bytes
            ne2000.registers.page0.rbcr_0_port.write(0);
            ne2000.registers.page0.rbcr_1_port.write(0);

            //=== STEP 4 ===//
            // initialize RCR
            // determines operation of the NIC during reception of a packet
            // and is used to program what types of packets to
            // accept.
            // RCR_AR : allow RUNT Packets (Packets < 64 Btyte)
            ne2000.registers.page0.rcr_port.write(
                (ReceiveConfigurationRegister::RCR_AR
                    | ReceiveConfigurationRegister::RCR_AB
                    | ReceiveConfigurationRegister::RCR_AM)
                    .bits(),
            );

            //=== STEP 5 ===//
            // Place the NIC in Loopback Mode (Mode 0)
            ne2000
                .registers
                .page0
                .tcr_port
                .write(TransmitConfigurationRegister::TCR_LB0.bits());

            //=== STEP 6 ===//
            // initialize the NIC's receive buffer
            // pstart and pstop define the size of the receive buffer (pstop - pstart = buffer size )
            ne2000.registers.page0.tpsr_port.write(TRANSMIT_START_PAGE);
            ne2000.registers.page0.pstart_port.write(RECEIVE_START_PAGE);
            ne2000.registers.page0.bnry_port.write(RECEIVE_START_PAGE + 1);
            ne2000.registers.page0.pstop_port.write(RECEIVE_STOP_PAGE);

            //=== STEP 7 ===//
            //  Clear ISR
            ne2000.registers.isr_port.lock().write(0xFF);

            //=== STEP 8 ===//
            // Initialize IMR
            // enables, disables interrupts
            // enable PacketReceived, PacketTransmit and Overwrite
            ne2000.registers.imr_port.lock().write(
                (InterruptMaskRegister::IMR_PRXE
                    | InterruptMaskRegister::IMR_PTXE
                    | InterruptMaskRegister::IMR_OVWE)
                    .bits(),
            );

            //=== STEP 9 ===//
            // Switch to P1, disable DMA and Stop the NIC
            ne2000
                .registers
                .command_port
                .write((CR::STOP | CR::PAGE_1).bits());

            // define array for saving the MAC Address
            let mut mac = [0u8; 6];

            // i) Initialize Physical Address Register: PAR0-PAR5
            // define the location of the data for the mac address
            // iterate through the ports to get the mac address
            // borrow the value
            let par = &ne2000.registers.page1.par;
            for (i, guard) in par.iter().enumerate() {
                let mut port = guard.lock();
                mac[i] = port.read();
            }

            // Write MAC address to PAR registers 
            for (i, guard) in par.iter().enumerate() {
                let mut port = guard.lock();
                port.write(mac[i]);
            }

            // located on Page 1
            // ii) Initialize Multicast Address Register: MAR0-MAR7 with 0xFF
            for port in ne2000.registers.page1.mar.iter_mut() {
                port.write(0xFF);
            }
            // p.156 http://www.bitsavers.org/components/national/_dataBooks/1988_National_Data_Communications_Local_Area_Networks_UARTs_Handbook.pdf#page=156
            CURRENT_NEXT_PAGE_POINTER = RECEIVE_START_PAGE + 1;

            // iii) Initialize Current Pointer: CURR
            ne2000
                .registers
                .page1
                .current_port
                .write(CURRENT_NEXT_PAGE_POINTER);

            // 10) Start the NIC
            ne2000
                .registers
                .command_port
                .write((CR::STOP_DMA | CR::STA | CR::PAGE_0).bits());

            //11) Initialize TCR and RCR
            ne2000.registers.page0.tcr_port.write(0);
            ne2000.registers.page0.rcr_port.write(
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

            /*scheduler().ready(Thread::new_kernel_thread(
                loop {
                    if ne2000.interrupts.rcv.load(Ordering::Relaxed) {
                        ne2000.receive_packet();
                        ne2000.interrupts.rcv.store(false, Ordering::Relaxed);
                    }
                },
                "Ne2k IRQ",
            ));*/

            ne2000
        }
    }

    // =============================================================================
    // ==== FUNCTION send_packet
    // =============================================================================
    // - the function is called by the consume function of TxToken and gets a datagram
    // as param.
    // - the function sets the internal registers of the nic for writing the packet
    //   to the local buffer of the nic
    // =============================================================================

    pub fn send_packet(&mut self, packet: &[u8]) {
        unsafe {
            // check, if the nic is ready for transmit
            while CR::from_bits_retain(self.registers.command_port.read()).contains(CR::TXP) {
                scheduler().sleep(1);
                info!("Transmit bit still set!");
            }

            //==== STEP 1 ====//
            // switch to page 0, enable nic, stop dma
            self.registers
                .command_port
                .write((CR::STA | CR::STOP_DMA | CR::PAGE_0).bits());

            // =============================================================================
            // dummy_read
            // =============================================================================
            // Usage: a dummy read is performed to ensure no data corruption occurs, 
            // when the nic first starts up 
            // Reference: p.13-14, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
            // =============================================================================

            info!("Start Dummy Read");


            // Save CRDA bit (Current Remote DMA Address)
            let old_crda: u16 = self.registers.page0.crda_0_p0.read() as u16
                | ((self.registers.page0.crda_1_p0.read() as u16) << 8);

            // Set RBCR > 0
            self.registers.page0.rbcr_0_port.write(0x01);
            self.registers.page0.rbcr_1_port.write(0x00);
            // Set RSAR to unused address
            self.registers.page0.rsar_0_port.write(TRANSMIT_START_PAGE);
            self.registers.page0.rsar_1_port.write(0);
            // Issue Dummy Remote READ Command
            self.registers
                .command_port
                .write((CR::STA | CR::REMOTE_READ | CR::PAGE_0).bits());

            // Mandatory Delay between Dummy Read and Write to ensure dummy read was successful
            // Wait until crda value has changed
            while old_crda
                == self.registers.page0.crda_0_p0.read() as u16
                    | ((self.registers.page0.crda_1_p0.read() as u16) << 8)
            {
                scheduler().sleep(1);
                info!("not equal")
            }
            info!("Finished Dummy Read");

            // =============================================================================
            // end dummy read
            // =============================================================================

            info!("Load packet size and enable remote write");
            //==== STEP 2 ====//
            // Load RBCR with packet size
            let packet_length = packet.len() as u32;
            let low = (packet_length & 0xFF) as u8;
            let high = (packet_length >> 8) as u8;
            self.registers.page0.rbcr_0_port.write(low);
            self.registers.page0.rbcr_1_port.write(high);

            //==== STEP 3 ====//
            // Remote DMA complete ? 
            // ref: https://wiki.osdev.org/Ne2000#Sending_a_Packet
            // Clear RDC Interrupt
            self.registers
                .isr_port
                .lock()
                .write(InterruptStatusRegister::ISR_RDC.bits());

            //==== STEP 4 ====//
            // Load RSAR with 0 (low bits) and Page Number (high bits)
            self.registers.page0.rsar_0_port.write(0);
            self.registers.page0.rsar_1_port.write(TRANSMIT_START_PAGE);

            //==== STEP 5 ====//
            // Set COMMAND to remote write
            self.registers
                .command_port
                .write((CR::STA | CR::REMOTE_WRITE | CR::PAGE_0).bits());

            //==== STEP 6 ====//
            // Write packet to remote DMA
            let data_port = &mut self.registers.data_port;
            for &data in packet {
                data_port.write(data);
            }

            //==== STEP 7 ====//
            // Poll ISR until remote DMA Bit is set
            while (self.registers.read_isr() & InterruptStatusRegister::ISR_RDC.bits()) == 0 {
                scheduler().sleep(1);
                info!("polling")
            }

            // Clear ISR RDC Interrupt Bit
            self.registers
                .isr_port
                .lock()
                .write(InterruptStatusRegister::ISR_RDC.bits());

            // Set TBCR Bits before Transmit and TPSR Bit
            self.registers.page0.tbcr_0_port_p0.write(low);
            self.registers.page0.tbcr_1_port_p0.write(high);
            self.registers.page0.tpsr_port.write(TRANSMIT_START_PAGE);


            // Set TXP Bit to send packet
            self.registers
                .command_port
                .write((CR::STA | CR::TXP | CR::STOP_DMA | CR::PAGE_0).bits());

            info!("finished send_packet fn");
        }
    }

    // =============================================================================
    // ==== FUNCTION receive_packet
    // =============================================================================
    // if a packet is received by the nic, process it
    // =============================================================================
    pub fn receive_packet(&mut self) {
        unsafe {
            // switch to page 1 to read curr register
            self.registers
                .command_port
                .write((CR::STA | CR::STOP_DMA | CR::PAGE_1).bits());

            // Read current register to prepare for the next packet
            let mut current = self.registers.page1.current_port.read();

            // switch back to Page 0
            self.registers
                .command_port
                .write((CR::STA | CR::STOP_DMA | CR::PAGE_0).bits());

            // as long as packets are there to be processed, loop
            while current != CURRENT_NEXT_PAGE_POINTER {
                // write size of header
                self.registers
                    .page0
                    .rbcr_0_port
                    .write(mem::size_of::<PacketHeader>() as u8);
                self.registers.page0.rbcr_1_port.write(0);
                self.registers.page0.rsar_0_port.write(0);
                self.registers.page0.rsar_1_port.write(CURRENT_NEXT_PAGE_POINTER);

                // enable remote Read
                self.registers
                    .command_port
                    .write((CR::STA | CR::REMOTE_READ | CR::PAGE_0).bits());

                // build the PacketHeader struct from the buffer ring
                // the nic always stores a packet header at the beginning of the first
                // buffer page which is used to store the received package
                // the nic itself attaches the a 4 Byte header to each packet
                // Reference: p.8, Section "Beginning of Reception", 
                // https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
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

                    let packet_length: u16 = packet_header.length as u16;

                    // Write packet length into RBCR
                    self.registers
                        .page0
                        .rbcr_0_port
                        .write((packet_length & 0xFF) as u8);

                    //self.registers.rbcr1.write(packet_header.length >> 8);
                    // fix overflow warning
                    self.registers.page0.rbcr_1_port.write((packet_length >> 8) as u8);

                    // Set RSAR0 to nicHeaderLength to skip the packet header during the read operation
                    self.registers
                        .page0
                        .rsar_0_port
                        .write(size_of::<PacketHeader>() as u8);

                    self.registers.page0.rsar_1_port.write(CURRENT_NEXT_PAGE_POINTER);

                    // issue remote read operation for reading the packet from the nics local buffer
                    self.registers
                        .command_port
                        .write((CR::STA | CR::REMOTE_READ | CR::PAGE_0).bits());

                    // Read Packet Data from I/O Port and write it into packet
                    //self.registers.data_port.read() as u8;
                    for i in 0..packet_header.length {
                        // slice indices must be of type usize
                        packet[i as usize] = self.registers.data_port.read();
                    }
                    let s: String = packet.iter().map(|&b| b as char).collect();
                    info!("{}", s);

                    // enqueue the packet in the receive_messages queue, 
                    //this queue gets processed by receive in smoltcp
                    self.receive_messages
                        .1
                        .try_enqueue(packet)
                        .expect("Error enqueuing packet");
                }

                // update pointers for the next package
                CURRENT_NEXT_PAGE_POINTER = packet_header.next_packet;
                if (packet_header.next_packet - 1) < RECEIVE_START_PAGE {
                    self.registers.page0.bnry_port.write(RECEIVE_STOP_PAGE - 1);
                } else {
                    self.registers
                        .page0
                        .bnry_port
                        .write(CURRENT_NEXT_PAGE_POINTER - 1);
                }

                // update the current variable for the next packet to be read
                self.registers
                    .command_port
                    .write((CR::STA | CR::STOP_DMA | CR::PAGE_1).bits());
                current = self.registers.page1.current_port.read();
                self.registers
                    .command_port
                    .write((CR::STA | CR::STOP_DMA | CR::PAGE_0).bits());
            }

            // clear the RDC Interrupt in the ISR (Remote DMA Operation has been completed)
            self.registers
                .isr_port
                .lock()
                .write(InterruptStatusRegister::ISR_RDC.bits());
        }
    }

    // =============================================================================
    // ==== FUNCTION read_mac
    // =============================================================================
    // read the mac address and return it
    // the mac is needed for checking if received packets
    // are addressed to the nic
    // =============================================================================
    pub fn read_mac(&self) -> EthernetAddress {
        //define mac array for storing the values from the PAR Registers
        let mut mac = [0u8; 6];

        unsafe {
            // switch to page 1 to access PAR 0..5
            // stop the nic
            // disable remote dma
            let mut registers = Registers::new(self.base_address);
            registers.command_port.write((CR::STOP | CR::PAGE_1).bits());

            // read 6 bytes (MAC address)
            // save the values of the PAR Registers in mac
            for (i, guard) in self.registers.page1.par.iter().enumerate() {
                let mut port = guard.lock();
                mac[i] = port.read();
            }

            // start the nic
                registers
                .command_port
                .write((CR::STA | CR::PAGE_0).bits());

        }
        // convert the data in the mac array to type EthernetAddress
        let mac_address = EthernetAddress::from_bytes(&mac);
        // return the actual MAC Address
        mac_address
    }

    // =============================================================================
    // ==== FUNCTION handle_overflow
    // =============================================================================
    // gets called, if the buffer ring is full
    // this is analogous to the nic datasheet
    // Reference: p.9-10, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
    // =============================================================================
    pub fn handle_overflow(&mut self) {
        info!("overflow");
        unsafe {
            // 1. save the value of the TXP Bit in CR
            let txp_bit = self.registers.command_port.read() & CR::TXP.bits();

            // 2. Issue stop command, stop NIC and DMA
            self.registers
                .command_port
                .write((CR::STOP | CR::PAGE_0).bits());

            // 3. wait for at least 1.6 ms according to the documentation, 
            // until transmit or receive operation has ended
            scheduler().sleep(1600);

            // 4. Clear RBCR0 and RBCR1
            self.registers.page0.rbcr_0_port.write(0);
            self.registers.page0.rbcr_1_port.write(0);

            // 5. read value of TXP bit, check if there was a 
            // transmission in progress when the stop command was issued
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
                .page0
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
            self.registers.page0.tcr_port.write(0);

            //11. if resend = 1, reset variable, reissue transmit command
            if resend == 1 {
                self.registers
                    .command_port
                    .write((CR::STA | CR::TXP | CR::STOP_DMA | CR::PAGE_0).bits());
            }
        }
    }

    // assign NIC to interrupt handler
    pub fn assign(device: Arc<Ne2000>) {
        // get the interrupt field in the ne2000 struct
        let interrupt = device.interrupt;
        //assign the interrupt handler to the given interrupt vector 
        interrupt_dispatcher().assign(interrupt, Box::new(Ne2000InterruptHandler::new(device)));
        // allow interrupt requests and handle them by the Advanced Programmable Interrupt controller
        // APIC distributes theses as Interrupt Messages to the local apics on the processors of the system
        // see: https://de.wikipedia.org/wiki/Advanced_Programmable_Interrupt_Controller
        apic().allow(interrupt);
    }
}

// implement the InterruptHandler
// creates a new Instance of Ne2000InterruptHandler
impl Ne2000InterruptHandler {
    pub fn new(device: Arc<Ne2000>) -> Self {
        Self { device }
    }
}

// =============================================================================
// ==== FUNCTION trigger
// =============================================================================
// gets called, if the nic receives or transmits a package
// or if an Buffer Overwrite occurs
// =============================================================================
impl InterruptHandler for Ne2000InterruptHandler {
    fn trigger(&self) {
        // a mutex is required for IMR and ISR, because these registers are also used by the
        // transmit and receive function, and Init routine
        if self.device.registers.isr_port.is_locked() {
            panic!("Interrupt status register is locked during interrupt!");
        }

        // clear Interrupt Mask Register
        // disables interrupts
        self.device.registers.write_imr(0);

        // Read interrupt status register (Each bit corresponds to an interrupt type or error)
        let status_reg = self.device.registers.read_isr();
        let status = InterruptStatusRegister::from_bits_retain(status_reg);

        // Check interrupt flags
        // Packet Reception Flag set (PRX) ? (Packet received?)
        if status.contains(InterruptStatusRegister::ISR_PRX) {
            // reset prx bit in isr
            unsafe {
                self.device
                    .registers
                    .isr_port
                    .lock()
                    .write(InterruptStatusRegister::ISR_PRX.bits());
                let device_ref: &Ne2000 = &self.device; // This is a shared reference
                // Use unsafe to get a mutable reference to the inner `Ne2000` object
                let device_mut =
                    // Convert from a shared reference to a mutable raw pointer
                    ptr::from_ref(device_ref)
                        .cast_mut() // Cast to a mutable pointer
                        .as_mut() // Convert the raw pointer back to a mutable reference
                        .unwrap(); // Unwrap to ensure it’s valid

                device_mut.receive_packet();
            };
            self.device.interrupts.rcv.store(true, Ordering::Relaxed);
        }

        // check for Packet Transmission Interrupt
        if status.contains(InterruptStatusRegister::ISR_PTX) {
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
            // call the method
            //let ovw =  self.device.interrupts.ovw;
            //ovw.store(true, Ordering::Relaxed)
            // `self.device` is of type `Arc<Ne2000>`, which is the shared reference
            // Use unsafe to get a mutable reference to the inner `Ne2000` object
            unsafe {
                let device_ref: &Ne2000 = &self.device; // This is a shared reference
                
                let device_mut = 
                // Convert from a shared reference to a mutable raw pointer
                    ptr::from_ref(device_ref)
                        .cast_mut() // Cast to a mutable pointer
                        .as_mut() // Convert the raw pointer back to a mutable reference
                        .unwrap(); // Unwrap to ensure it’s valid
                device_mut.handle_overflow();
            }
            
        }

        // re-enable Interrupts (22.07.2025)
        unsafe {
            self.device.registers.imr_port.lock().write(
                (InterruptMaskRegister::IMR_PRXE
                    | InterruptMaskRegister::IMR_PTXE
                    | InterruptMaskRegister::IMR_OVWE)
                    .bits(),
            );
        }
    }
}
