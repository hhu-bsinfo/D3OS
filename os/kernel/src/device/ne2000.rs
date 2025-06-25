use crate::{apic, interrupt_dispatcher, pci_bus, process_manager, scheduler};
use bitflags::bitflags;
use core::{ptr, slice};
use log::info;
use pci_types::{CommandRegister, EndpointHeader};
use smallmap::Page;
use smoltcp::phy;
use smoltcp::phy::{DeviceCapabilities, Medium};
use smoltcp::time::Instant;
use smoltcp::wire::EthernetAddress;
use spin::{Mutex, RwLock};
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

static RESET: u8 = 0x1F;
static TRANSMIT_START_PAGE: u8 = 0x40;

/**
 * Reception Buffer Ring Start Page
 * http://www.osdever.net/documents/WritingDriversForTheDP8390.pdf
 * Page 4 PSTART
 */
static RECEIVE_START_PAGE: u8 = 0x46;

/**
 * Reception Buffer Ring End
 * P.4 PSTOP http://www.osdever.net/documents/WritingDriversForTheDP8390.pdf
 * Accessed:
 */
static RECEIVE_STOP_PAGE: u8 = 0x80;

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

struct Registers {
    reset_port: Port<u8>,
    command_port: Port<u8>,
    rsar0: Port<u8>,
    rsar1: Port<u8>,
    rbcr0: Port<u8>,
    rbcr1: Port<u8>,
    data_port: Port<u8>,
    isr_port: Port<u8>,
    rst_port: Port<u8>,
    imr_port: Port<u8>,
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
        Self {
            reset_port: Port::new(base_address + 0x1F),
            command_port: Port::new(base_address + 0x00),
            rsar0: Port::new(base_address + 0x08),
            rsar1: Port::new(base_address + 0x09),
            rbcr0: Port::new(base_address + 0x0A),
            rbcr1: Port::new(base_address + 0x0B),
            data_port: Port::new(base_address + 0x10),
            isr_port: Port::new(base_address + 0x07),
            rst_port: Port::new(base_address + 0x80),
            imr_port: Port::new(base_address + 0x0F),
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

bitflags! {
    pub struct PageRegisters : u8 {
        const COMMAND     = 0x00;         //** R|W COMMAND used for P0, P1, P2 */
        // P0 Write
        const P0_PSTART   = 0x01;        //* W Page Start Register  */
        const P0_PSTOP    = 0x02;        //* W Page Stop Register  */
        const P0_BNRY     = 0x03;        //* R|W Boundary Pointer  P0 */
        const P0_TPSR     = 0x04;        //* W Transmit Page Start Address  */
        const P0_TBCR0    = 0x05;        //* W Transmit Byte Count Register 0  */
        const P0_TBCR1    = 0x06;        //* W Transmit Byte Count Register 1  */
        const P0_ISR      = 0x07;        //* R|W Interrupt Status Register P0 */
        const P0_RSAR0    = 0x08;        //* W Remote Start Address Register 0 */
        const P0_RSAR1    = 0x09;        //* W Remote Start Address Register 1 */
        const P0_RBCR0    = 0x0A;        //* W Remote Byte Count Register 0 */
        const P0_RBCR1    = 0x0B;        //* W Remote Byte Count Register 1 */
        const P0_RCR      = 0x0C;        //* W Receive Configuration Register */
        const P0_TCR      = 0x0D;        //* W Transmit Configuration Register*/
        const P0_DCR      = 0x0E;        //* W Data Configuration Register */
        const P0_IMR      = 0x0F;        //* W Interrupt Mask Register */
        // P0 Read Registers*/
        const P0_CLDA0    = 0x01;        //** R Current Local DMA Address 0  */
        const P0_CLDA1    = 0x02;        //** R Current Local DMA Address 1  */
        const P0_TSR      = 0x04;        //** R Transmit Status Register  */
        const P0_NCR      = 0x05;        //** R Number of Collisions Register  */
        const P0_FIFO     = 0x06;        //** R FIFO */
        const P0_CRDA0    = 0x08;        //** R Current Remote DMA Address 0 */
        const P0_CRDA1    = 0x09;        //** R Current Remote DMA Address 1 */
        const P0_RSR      = 0x0C;        //** R Receive Status Register */
        const P0_CNTR0    = 0x0D;        //** R Tally Counter 0 (Frame Alignment Errors) */
        const P0_CNTR1    = 0x0E;        //** R Tally Counter 1 (CRC Errors) */
        const P0_CNTR2    = 0x0F;        //** R Tally Counter 2 (Missed Packet Error) */
        // P1 Read and Write Registers */
        const P1_PAR0     = 0x01;        //* R|W Physical Address Register 0 */
        const P1_PAR1     = 0x02;        //* R|W Physical Address Register 1 */
        const P1_PAR2     = 0x03;        //* R|W Physical Address Register 2 */
        const P1_PAR3     = 0x04;        //* R|W Physical Address Register 3 */
        const P1_PAR4     = 0x05;        //* R|W Physical Address Register 4 */
        const P1_PAR5     = 0x06;        //* R|W Physical Address Register 5 */
        const P1_CURR     = 0x07;        //* R|W Current Page Register */
        const P1_MAR0     = 0x08;        //* R|W Multicast Address Register 0 */
        const P1_MAR1     = 0x09;        //* R|W Multicast Address Register 1 */
        const P1_MAR2     = 0x0A;        //* R|W Multicast Address Register 2 */
        const P1_MAR3     = 0x0B;        //* R|W Multicast Address Register 3 */
        const P1_MAR4     = 0x0C;        //* R|W Multicast Address Register 4 */
        const P1_MAR5     = 0x0D;        //* R|W Multicast Address Register 5 */
        const P1_MAR6     = 0x0E;        //* R|W Multicast Address Register 6 */
        const P1_MAR7     = 0x0F;        //* R|W Multicast Address Register 7 */
        // P2 Registers are only fo/ diagnostic purposes. They should not be accessed during normal operation */
        // P2 Write Registers */
        const P2_CLDA0    = 0x01;        //* W Current Local DMA Address 0 */
        const P2_CLDA1    = 0x02;        //* W Current Local DMA Address 1 */
        const P2_RNPP     = 0x03;        //* R|W Remote Next Packet Pointer */
        const P2_LNPP     = 0x05;        //* R|W Local Next Packet Pointer */
        const P2_UPPER    = 0x06;        //* R|W Address Counter (Upper) */
        const P2_LOWER    = 0x07;        //* R|W Address Counter (Lower) */
        // P2 Read */
        const P2_PSTART   = 0x01;        //* R Page Start Register */
        const P2_PSTOP    = 0x02;        //* R Page Stop Register */
        const P2_TPSR     = 0x04;        //* R Transmit Page Start Address */
        const P2_RCR      = 0x0C;        //* R Receive Configuration Register */
        const P2_TCR      = 0x0D;        //* R Transmit Configuration Register */
        const P2_DCR      = 0x0E;        //* R Data Configuration Register */
        const P2_IMR      = 0x0F;        //* R Interrupt Mask Register */
    }
}

// Command Register
bitflags! {
    pub struct CR :u8 {
        const STP = 0x01; /// STOP
        const STA = 0x02; /** START */
        const TXP = 0x04; /** Transmit Packet */
        const RD_0 = 0x08; /** Remote DMA Command 0 */
        const RD_1 = 0x10; /** Remote DMA Command 1 */
        const RD_2 = 0x20; /** Remote DMA Command 2*/
        const PS_0 = 0x40; /** Page Select PS0 */
        const PS_1 = 0x80; /** Page Select PS1 */
        /** Page Selection Commands */
        const PAGE_0 = 0x00;
        const PAGE_1 = 0x40;
        const PAGE_2 = 0x80;
        /** Remote DMA Commands */
        const REMOTE_READ = 0x08;
        const REMOTE_WRITE = 0x10;
        const SEND_PACKET = 0x08 | 0x10;
        const STOP_DMA = 0x20;
        const STOP = 0x01 | 0x08;
    }
}

bitflags! {

    pub struct InterruptStatusRegister : u8 {
        const ISR_PRX = 0x01;
        const ISR_PTX = 0x02;
        const ISR_RXE = 0x04;
        const ISR_TXE = 0x08;
        const ISR_OVW = 0x10;
        const ISR_CNT = 0x20;
        const ISR_RDC = 0x40;
        const ISR_RST = 0x80;  // Reset Status
    }
}

bitflags! {

    pub struct InterruptMaskRegister : u8 {
        const IMR_PRXE = 0x01;
        const IMR_PTXE = 0x02;
        const IMR_RXEE = 0x04;
        const IMR_TXEE = 0x08;
        const IMR_OVWE = 0x10;
        const IMR_CNTE = 0x20;
        const IMR_RDCE = 0x40;
    }
}

//Data Configuration Register as defined in DP8390D
//P.22 https://datasheetspdf.com/pdf-file/549771/NationalSemiconductor/DP8390D/1
// Accessed: 2024-03-29
bitflags! {
    pub struct DataConfigurationRegister : u8 {
        const DCR_WTS = 0x01;
        const DCR_BOS = 0x02;
        const DCR_LAS = 0x04;
        const DCR_LS  = 0x08;
        const DCR_AR  = 0x10;
        const DCR_FT0 = 0x20;
        const DCR_FT1 = 0x40;
    }
}

// Transmit Configuration Register as defined in DP8390D
//P.23 https://datasheetspdf.com/pdf-file/549771/NationalSemiconductor/DP8390D/1
//Accessed: 2024-03-29

bitflags! {
    pub struct TransmitConfigurationRegister : u8 {
        const TCR_CRC  = 0x01;  //Inhibit CRC
        const TCR_LB0  = 0x02;  //Encoded Loop-back Control
        const TCR_LB1  = 0x04;  //Encoded Loop-back Control
        const TCR_ATD  = 0x08;  //Auto Transmit Disable
        const TCR_OFST = 0x10;  //Collision Offset Enable
    }
}

//* Transmit Status Register as defined in DP8390D
//* P. 24https://datasheetspdf.com/pdf-file/549771/NationalSemiconductor/DP8390D/1
//* Accessed: 2024-03-29
bitflags! {

    pub struct TransmitStatusRegister : u8 {
        const TSR_PTX = 0x01; // Packet Transmit */
        const TSR_COL = 0x02; // Transmit Collided */
        const TSR_ABT = 0x04; // Transmit Aborted */
        const TSR_CRS = 0x08; // Carrier Sense Lost */
        const TSR_FU  = 0x10; // FIFO Under-run */
        const TSR_CDH = 0x20; // CD Heartbeat */
        const TSR_OWC = 0x40; // Out of Window Collision */
    }
}

//* Receive Configuration Register as defined in DP8390D
//* P.25 https://datasheetspdf.com/pdf-file/549771/NationalSemiconductor/DP8390D/1
//* Accessed: 2024-03-29

bitflags! {

    pub struct ReceiveConfigurationRegister : u8 {
        const RCR_SEP = 0x01; // Save Error Packets */
        const RCR_AR  = 0x02; // Accept Runt Packets */
        const RCR_AB  = 0x04; // Accept Broadcast */
        const RCR_AM  = 0x08; // Accept Multicast */
        const RCR_PRO = 0x10; // Promiscuous Physical */
        const RCR_MON = 0x20; // Monitor Mode */
    }
}

// Receive Status Register as defined in DP8390D
// P.26 https://datasheetspdf.com/pdf-file/549771/NationalSemiconductor/DP8390D/1
// Accessed: 2024-03-29

bitflags! {
    pub struct ReceiveStatusRegister : u8 {
        const RSR_PRX = 0x01; //** Packet Received Intact */
        const RSR_CRC = 0x02; //** CRC Error */
        const RSR_FAE = 0x04; //** Frame Alignment Error */
        const RSR_FO  = 0x08; //** FIFO Overrun */
        const RSR_MPA = 0x10; //** Missed Packet*/
        const RSR_PHY = 0x20; //** Physical/Multicast Address */
        const RSR_DIS = 0x40; //** Receiver Disabled */
        const RSR_DFR = 0x80;  //** Deferring */
    }
}

pub struct Ne2000 {
    base_address: u16,
    registers: Registers,
    par_registers: ParRegisters,
}
//& borrowing the Struct Ne2000
// 'a lifetime annotation
// implementation is orientated on the rtl8139.rs module

// changed to mut because send packet expects mutable self reference
//
pub struct Ne2000TxToken<'a> {
    device: &'a Ne2000,
}

// implementation is orientated on the rtl8139.rs module
// generate new transmission token
// a token to send a single network packet
// see: https://docs.rs/smoltcp/latest/smoltcp/phy/trait.TxToken.html

impl<'a> Ne2000TxToken<'a> {
    pub fn new(device: &'a Ne2000) -> Self {
        Self { device }
    }
}

// implementation is orientated on the rtl8139.rs module
// len: size of packet
impl<'a> phy::TxToken for Ne2000TxToken<'a> {
    // consumes the token to send a single network packet
    // constructs buffer (size len) -> calls passed closure f
    // in the closure a valid network packet should be constructed
    // when closure returns, packet gets send out
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        // Allocate and fill local buffer
        // max. buffer size is 1514 (see documentation )
        // TODO: add reference in manual for this
        //let mut buffer = [0u8; 1514];
        //let data = &mut buffer[..len];
        //let result = f(data);

        // call send method using the NE2000
        // TODO: implement send Methode
        //self.device.send_packet(data);

        result
    }
}

//for the moment not implemented
pub struct Ne2000RxToken;

impl phy::RxToken for Ne2000RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        // Return empty slice
        f(&[])
    }
}

impl phy::Device for Ne2000 {
    type RxToken<'a>
        = Ne2000RxToken
    where
        Self: 'a;
    type TxToken<'a>
        = Ne2000TxToken<'a>
    where
        Self: 'a;

    // called by smoltcp, when polling for new packets
    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        // disable receive for now; only transmit exists
        //self.transmit(_timestamp).map(|tx| (Ne2000RxToken, tx))
        None
    }

    // Converts &mut self to &Ne2000 safely.
    // Needed because RxToken and TxToken store a shared reference to the driver (not &mut self). See RTL8139 impl
    // Returns a TxToken, which accepts the packet contents
    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        let device = unsafe { ptr::from_ref(self).as_ref()? };
        Some(Ne2000TxToken::new(device))
    }

    // define what the device supports
    //max_burst_size = only send one packet at a time
    // medium = send packet over Ethernet
    // max_transmission_unit = define max. size of a packet
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1514;
        caps.max_burst_size = Some(1);
        caps.medium = Medium::Ethernet;

        caps
    }
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

        let mut ne2000 = Self {
            registers: Registers::new(base_address),
            base_address: base_address,
            par_registers: ParRegisters::new(base_address),
        };

        //ne2000.init();
        //let mut buffer = [0u8; 1514];
        //let data = &mut buffer[..1514];
        //ne2000.send_packet(data);

        //}

        //pub fn init(&mut self) {
        info!("Powering on device");
        unsafe {
            //command_port.write(0x02);
            //let registers = &mut self.registers;
            //let j = self.registers.isr_port.read();
            //info!("ISR: {}", j);
            info!("Resetting Device NE2000");

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
            while (ne2000.registers.isr_port.read() & 0x80) == 0 {
                info!("Reset in Progress");
            }
            info!("Ne2000 reset complete");

            info!("Initializing Registers of Device Ne2000");

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

            // Buffer Initialization
            //baseRegister.writeByte(P0_TPSR, TRANSMIT_START_PAGE);
            //baseRegister.writeByte(P0_PSTART, RECEIVE_START_PAGE);
            //baseRegister.writeByte(P0_BNRY, RECEIVE_START_PAGE + 1);
            //baseRegister.writeByte(P0_PSTOP, RECEIVE_STOP_PAGE);

            ne2000.registers.tpsr_port.write(TRANSMIT_START_PAGE);
            ne2000.registers.pstart_port.write(RECEIVE_START_PAGE);
            ne2000.registers.bnry_port.write(RECEIVE_START_PAGE + 1);
            ne2000.registers.pstop_port.write(RECEIVE_STOP_PAGE);

            //  Clear ISR
            ne2000.registers.isr_port.write(0xFF);

            // Initialize IMR
            ne2000.registers.imr_port.write(
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

            /* 9) i) Initialize Physical Address Register: PAR0-PAR5
            each mac address bit is written two times into the buffer
            */
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
            let current_next_page_pointer = RECEIVE_START_PAGE + 1;

            /* 9) iii) Initialize Current Pointer: CURR */
            ne2000.registers.curr.write(current_next_page_pointer);

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
            info!("Finished Initialization");
        }
        let dummy: [u8; 0] = [];
        ne2000.send_packet(&dummy);
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
                data_port.write(data);
            }

            // 6) Poll ISR until remote DMA Bit is set
            while (self.registers.isr_port.read() & InterruptStatusRegister::ISR_RDC.bits()) == 0 {
                scheduler().sleep(1);
                info!("polling")
            }

            // 7) Clear ISR RDC Interrupt
            self.registers
                .isr_port
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
