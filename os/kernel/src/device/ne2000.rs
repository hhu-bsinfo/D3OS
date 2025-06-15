use crate::{apic, interrupt_dispatcher, pci_bus, process_manager, scheduler};
use bitflags::bitflags;
use log::info;
use pci_types::{CommandRegister, EndpointHeader};
use smoltcp::wire::EthernetAddress;
use spin::{Mutex, RwLock};
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

static RESET: u8 = 0x1F;

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
    registers: Registers,
}

impl Ne2000 {
    pub fn new(pci_device: &RwLock<EndpointHeader>) -> Self {
        info!("Configuring PCI registers");
        //Self { base_address }
        let pci_config_space = pci_bus().config_space();
        let pci_device = pci_device.write();

        let bar0 = pci_device
            .bar(0, pci_bus().config_space())
            .expect("Failed to read base address!");

        let base_address = bar0.unwrap_io() as u16;
        let ne2000 = Self { base_address };
        info!("NE2000 base address: [0x{:x}]", base_address);

        ne2000
    }

    pub fn init(&self) -> Self {
        //pub fn read_mac(&self) -> EthernetAddress {

        let ne2000 = Self {};
        unsafe {
            // Define ports

            // reset port
            let mut reset_port = Port::<u8>::new(self.base_address + 0x1F);
            let mut command_port = Port::<u8>::new(self.base_address + 0x00);
            let mut rsar0 = Port::<u8>::new(self.base_address + 0x08);
            let mut rsar1 = Port::<u8>::new(self.base_address + 0x09);
            let mut rbcr0 = Port::<u8>::new(self.base_address + 0x0A);
            let mut rbcr1 = Port::<u8>::new(self.base_address + 0x0B);
            let mut data_port = Port::<u8>::new(self.base_address + 0x10);
            let mut isr_port = Port::<u8>::new(self.base_address + 0x07);
            let mut rst_port = Port::<u8>::new(self.base_address + 0x80);
            let mut imr_port = Port::<u8>::new(self.base_address + 0x0F);
            let mut dcr_port = Port::<u8>::new(self.base_address + 0x0E);
            let mut tcr_port = Port::<u8>::new(self.base_address + 0x0D);
            let mut rcr_port = Port::<u8>::new(self.base_address + 0x0C);
            let mut tpsr_port = Port::<u8>::new(self.base_address + 0x04);
            let mut pstart_port = Port::<u8>::new(self.base_address + 0x01);
            let mut pstop_port = Port::<u8>::new(self.base_address + 0x02);
            let mut bnry_port = Port::<u8>::new(self.base_address + 0x03);

            info!("Powering on device");
            //command_port.write(0x02);
            let j = isr_port.read();
            info!("ISR: {}", j);
            let d = command_port.read();
            info!("CR: {}", d);
            let e = imr_port.read();
            info!("IMR: {}", e);
            let f = dcr_port.read();
            info!("DCR: {}", f);
            let g = tcr_port.read();
            info!("TCR: {}", g);
            info!("Resetting Device NE2000");

            //Reset the NIC
            // Clears the Registers CR, ISR, IMR, DCR, TCR (see NS32490D.pdf, p.29, 11.0 Initialization Procedure)
            // this ensures, that the Registers are cleared and no undefined behavior can happen

            // From C++ Ne2000
            /** Wait until Reset Status is 0 */
            //while(!(baseRegister.readByte(P0_ISR) & ISR_RST)) {
            //Util::Async::Thread::sleep(Util::Time::Timestamp::ofMilliseconds(1));
            //}
            // Wait for the reset to complete
            //reset_port.write(0x00);

            // just doing the read operation enables the reset, a write is not necessary, but the bits dont get set correctly
            // see spec in PDF
            //TODO:, add comments what registers are affected and which bits are set
            let a = reset_port.read();
            info!("1: 0x{:X}", a);
            //reset_port.write(a);
            let c = isr_port.read();
            info!("ISR: 0x{:X}", c);

            while (isr_port.read() & 0x80) == 0 {
                //info!("Reset in Progress");
            }
            isr_port.write(c);
            info!("NE2000 reset complete");

            /*let j = isr_port.read();
            info!("ISR: 0x{:X}", j);
            let d = command_port.read();
            info!("CR: {}", d);
            let e = imr_port.read();
            info!("IMR: {}", e);
            let f = dcr_port.read();
            info!("DCR: 0x{:X}", f);
            let g = tcr_port.read();
            info!("TCR: {}", g);*/

            info!("Initializing Device NE2000");

            // Initialize CR Register
            //command_port.write(0x21);
            //info!("cr: {}", command_port.read());
            //scheduler().sleep(100);

            // Initialize DCR Register
            dcr_port.write(0x58);

            // clear RBCR1,0
            //RBCR0,1 : indicates the length of the block in bytes
            // MAC address has length of 6 Bytes
            rbcr0.write(0);
            rbcr1.write(0);
            info!("rb0: {}", rbcr0.read());
            info!("rb1: {}", rbcr1.read());

            // initialize RCR
            rcr_port.write(0x14);

            // Place the NIC in Loopback Mode (Mode 1)
            tcr_port.write(0x02);

            // Buffer Initialization
            //baseRegister.writeByte(P0_TPSR, TRANSMIT_START_PAGE);
            //baseRegister.writeByte(P0_PSTART, RECEIVE_START_PAGE);
            //baseRegister.writeByte(P0_BNRY, RECEIVE_START_PAGE + 1);
            //baseRegister.writeByte(P0_PSTOP, RECEIVE_STOP_PAGE);

            //tpsr_port.write();

            //Set up Remote DMA to read from address 0x0000
            // RSAR0,1 : points to the start of the block of data to be transfered
            rsar0.write(0);
            rsar1.write(0);
            info!("rsa0: {}", rsar0.read());
            info!("rsa1: {}", rsar1.read());
            rbcr0.write(6);
            rbcr1.write(0);
            info!("rb0: {}", rbcr0.read());
            info!("rb1: {}", rbcr1.read());

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
            //info!("Page: ({}) ", cr);
        }
        ne2000
    }

    pub fn read_mac(&self) -> [u8; 6] {
        //pub fn read_mac(&self) -> EthernetAddress {
        let mut mac = [0u8; 6];
        let mut mac2 = [0u8; 6];

        unsafe {
            // Define ports

            // reset port
            let mut reset_port = Port::<u8>::new(self.base_address + 0x1F);
            let mut command_port = Port::<u8>::new(self.base_address + 0x00);
            let mut rsar0 = Port::<u8>::new(self.base_address + 0x08);
            let mut rsar1 = Port::<u8>::new(self.base_address + 0x09);
            let mut rbcr0 = Port::<u8>::new(self.base_address + 0x0A);
            let mut rbcr1 = Port::<u8>::new(self.base_address + 0x0B);
            let mut data_port = Port::<u8>::new(self.base_address + 0x10);
            let mut isr_port = Port::<u8>::new(self.base_address + 0x07);
            let mut rst_port = Port::<u8>::new(self.base_address + 0x80);
            let mut imr_port = Port::<u8>::new(self.base_address + 0x0F);
            let mut dcr_port = Port::<u8>::new(self.base_address + 0x0E);
            let mut tcr_port = Port::<u8>::new(self.base_address + 0x0D);
            let mut rcr_port = Port::<u8>::new(self.base_address + 0x0C);
            let mut tpsr_port = Port::<u8>::new(self.base_address + 0x04);
            let mut pstart_port = Port::<u8>::new(self.base_address + 0x01);
            let mut pstop_port = Port::<u8>::new(self.base_address + 0x02);
            let mut bnry_port = Port::<u8>::new(self.base_address + 0x03);

            //Set up Remote DMA to read from address 0x0000
            // RSAR0,1 : points to the start of the block of data to be transfered
            rsar0.write(0);
            rsar1.write(0);
            info!("rsa0: {}", rsar0.read());
            info!("rsa1: {}", rsar1.read());
            rbcr0.write(6);
            rbcr1.write(0);
            info!("rb0: {}", rbcr0.read());
            info!("rb1: {}", rbcr1.read());

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
                mac2[i] = port.read();
            }

            let address3 = EthernetAddress::from_bytes(&mac2);
            info!("after mac 2 ({})", address3);

            // check if on correct Page (on Page 1 are the PARs Registers for the MAC Adress)

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

            let address2 = EthernetAddress::from_bytes(&mac);
            info!("after ({})", address2);
        }
        mac
    }
}
