// =============================================================================
// FILE        : register_flags.rs
// AUTHOR      : Johann Spenrath <johann.spenrath@hhu.de>
// DESCRIPTION : defines the registers of the NE2000 and their corresponding bits,
//               which control the Ne2000's functionalities
//               use these Constants for setting the correct bits in a register
//
// NOTES:
//
//
// =============================================================================
// DEPENDENCIES:
// =============================================================================
use bitflags::bitflags;
// =============================================================================

pub mod page_registers_offsets {
    pub const COMMAND: u16 = 0x00; // R|W COMMAND used for P0, P1, P2
    pub const RESET: u16 = 0x1F;
    // define offset for i/o port
    pub const DATA: u16 = 0x10;
    // P0 Write Registers
    pub const P0_PSTART: u16 = 0x01; // W Page Start Register
    pub const P0_PSTOP: u16 = 0x02; // W Page Stop Register
    pub const P0_BNRY: u16 = 0x03; // R|W Boundary Pointer  P0
    pub const P0_TPSR: u16 = 0x04; // W Transmit Page Start Address
    pub const P0_TBCR0: u16 = 0x05; // W Transmit Byte Count Register 0
    pub const P0_TBCR1: u16 = 0x06; // W Transmit Byte Count Register 1
    pub const P0_ISR: u16 = 0x07; // R|W Interrupt Status Register P0
    pub const P0_RSAR0: u16 = 0x08; // W Remote Start Address Register 0
    pub const P0_RSAR1: u16 = 0x09; // W Remote Start Address Register 1
    pub const P0_RBCR0: u16 = 0x0A; // W Remote Byte Count Register 0
    pub const P0_RBCR1: u16 = 0x0B; // W Remote Byte Count Register 1
    pub const P0_RCR: u16 = 0x0C; // W Receive Configuration Register
    pub const P0_TCR: u16 = 0x0D; // W Transmit Configuration Register
    pub const P0_DCR: u16 = 0x0E; // W Data Configuration Register
    pub const P0_IMR: u16 = 0x0F; // W Interrupt Mask Register
    // P0 Read Registers
    pub const P0_CLDA0: u16 = 0x01; // R Current Local DMA Address 0
    pub const P0_CLDA1: u16 = 0x02; // R Current Local DMA Address 1
    pub const P0_TSR: u16 = 0x04; // R Transmit Status Register
    pub const P0_NCR: u16 = 0x05; // R Number of Collisions Register
    pub const P0_FIFO: u16 = 0x06; // R FIFO */
    pub const P0_CRDA0: u16 = 0x08; // R Current Remote DMA Address 0
    pub const P0_CRDA1: u16 = 0x09; // R Current Remote DMA Address 1
    pub const P0_RSR: u16 = 0x0C; // R Receive Status Register
    pub const P0_CNTR0: u16 = 0x0D; // R Tally Counter 0 (Frame Alignment Errors)
    pub const P0_CNTR1: u16 = 0x0E; // R Tally Counter 1 (CRC Errors)
    pub const P0_CNTR2: u16 = 0x0F; // R Tally Counter 2 (Missed Packet Error)
    // P1 Read and Write Registers
    pub const P1_PAR0: u16 = 0x01; // R|W Physical Address Register 0
    pub const P1_PAR1: u16 = 0x02; // R|W Physical Address Register 1
    pub const P1_PAR2: u16 = 0x03; // R|W Physical Address Register 2
    pub const P1_PAR3: u16 = 0x04; // R|W Physical Address Register 3
    pub const P1_PAR4: u16 = 0x05; // R|W Physical Address Register 4
    pub const P1_PAR5: u16 = 0x06; // R|W Physical Address Register 5
    pub const P1_CURR: u16 = 0x07; // R|W Current Page Register 
    pub const P1_MAR0: u16 = 0x08; // R|W Multicast Address Register 0
    pub const P1_MAR1: u16 = 0x09; // R|W Multicast Address Register 1
    pub const P1_MAR2: u16 = 0x0A; // R|W Multicast Address Register 2
    pub const P1_MAR3: u16 = 0x0B; // R|W Multicast Address Register 3
    pub const P1_MAR4: u16 = 0x0C; // R|W Multicast Address Register 4
    pub const P1_MAR5: u16 = 0x0D; // R|W Multicast Address Register 5
    pub const P1_MAR6: u16 = 0x0E; // R|W Multicast Address Register 6
    pub const P1_MAR7: u16 = 0x0F; // R|W Multicast Address Register 7 
    // P2 Registers are only for diagnostic purposes.
    // P2 Write Registers
    pub const P2_CLDA0: u16 = 0x01; // W Current Local DMA Address 0 
    pub const P2_CLDA1: u16 = 0x02; // W Current Local DMA Address 1 
    pub const P2_RNPP: u16 = 0x03; // R|W Remote Next Packet Pointer 
    pub const P2_LNPP: u16 = 0x05; // R|W Local Next Packet Pointer 
    pub const P2_UPPER: u16 = 0x06; // R|W Address Counter (Upper) 
    pub const P2_LOWER: u16 = 0x07; // R|W Address Counter (Lower) 
    // P2 Read Registers
    pub const P2_PSTART: u16 = 0x01; // R Page Start Register 
    pub const P2_PSTOP: u16 = 0x02; // R Page Stop Register 
    pub const P2_TPSR: u16 = 0x04; // R Transmit Page Start Address 
    pub const P2_RCR: u16 = 0x0C; // R Receive Configuration Register 
    pub const P2_TCR: u16 = 0x0D; // R Transmit Configuration Register 
    pub const P2_DCR: u16 = 0x0E; // R Data Configuration Register 
    pub const P2_IMR: u16 = 0x0F; // R Interrupt Mask Register 
}

// =============================================================================
// Command Register
// Usage: switch between pages, start/stop the nic, enable/disable DMA
// Reference: p.20, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
// =============================================================================
bitflags! {
    pub struct CR :u8 {
        const STP = 0x01; // STOP
        const STA = 0x02; // START
        const TXP = 0x04; // Transmit Packet
        const RD_0 = 0x08; // Remote DMA Command 0
        const RD_1 = 0x10; // Remote DMA Command 1
        const RD_2 = 0x20; // Remote DMA Command 2
        const PS_0 = 0x40; // Page Select PS0
        const PS_1 = 0x80; // Page Select PS1
        // Page Selection Commands
        const PAGE_0 = 0x00;
        const PAGE_1 = 0x40;
        const PAGE_2 = 0x80;
        // Remote DMA Commands
        const REMOTE_READ = 0x08;
        const REMOTE_WRITE = 0x10;
        const SEND_PACKET = 0x08 | 0x10;
        const STOP_DMA = 0x20;
        // Stop nic and dma
        const STOP = 0x01 | 0x20;
    }
}

// =============================================================================
// InterruptStatusRegister
// Usage: - get Status of the Interrupts which occured during operation of
//          the card
//        - interrupts are cleared by writing a "1" into the register
// Reference: p.20, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
// =============================================================================

bitflags! {
    pub struct InterruptStatusRegister : u8 {
        const ISR_PRX = 0x01; //packet received
        const ISR_PTX = 0x02; //packet transmitted
        const ISR_RXE = 0x04; //receive error
        const ISR_TXE = 0x08; // transmit error
        const ISR_OVW = 0x10; // overwrite warning
        const ISR_CNT = 0x20; // counter overflow
        const ISR_RDC = 0x40; // dma complete
        const ISR_RST = 0x80;  // Reset Status
    }
}

// =============================================================================
// InterruptMaskRegister
// Usage: enable / disable interrupts
// Reference: p.21, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
// =============================================================================
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

// =============================================================================
// Data Configuration Register
// Usage: control FIFO treshholds, byte order, loopback mode
// Reference: p.22, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
// =============================================================================
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

// =============================================================================
// Transmit Configuration Register
// Usage: control actions of the transmitter section of the nic
// Reference: p.23, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
// =============================================================================
bitflags! {
    pub struct TransmitConfigurationRegister : u8 {
        const TCR_CRC  = 0x01;  //Inhibit CRC
        const TCR_LB0  = 0x02;  //Encoded Loop-back Control
        const TCR_LB1  = 0x04;  //Encoded Loop-back Control
        const TCR_ATD  = 0x08;  //Auto Transmit Disable
        const TCR_OFST = 0x10;  //Collision Offset Enable
    }
}

// =============================================================================
// Transmit Status Register
// Usage: - get the Status of Events, which happened during transmission of a packet
//        - cleared when next transmission is initiated
// Reference: p.24, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
// =============================================================================
bitflags! {
    pub struct TransmitStatusRegister : u8 {
        const TSR_PTX = 0x01; // Packet Transmit
        const TSR_COL = 0x02; // Transmit Collided
        const TSR_ABT = 0x04; // Transmit Aborted
        const TSR_CRS = 0x08; // Carrier Sense Lost
        const TSR_FU  = 0x10; // FIFO Under-run
        const TSR_CDH = 0x20; // CD Heartbeat
        const TSR_OWC = 0x40; // Out of Window Collision
    }
}

// =============================================================================
// Receive Configuration Register
// Usage: - set, which types of packets should be accepted by the nic
//        - define operations of the nic during reception
// Reference: p.25, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
// =============================================================================
bitflags! {
    pub struct ReceiveConfigurationRegister : u8 {
        const RCR_SEP = 0x01; // Save Error Packets
        const RCR_AR  = 0x02; // Accept Runt Packets
        const RCR_AB  = 0x04; // Accept Broadcast
        const RCR_AM  = 0x08; // Accept Multicast
        const RCR_PRO = 0x10; // Promiscuous Physical, if set all packets will be accepeted regardless of what is saved in the address part
        const RCR_MON = 0x20; // Monitor Mode
    }
}

// =============================================================================
// Receive Status Register
// Usage: - record status of the received packet (errors, physical or multicast address)
//        - if packet received successful -> contents of the register will be written to buffer memory
//        - if not : write to memory at the head of the erroneous packet
// Reference: p.26, https://web.archive.org/web/20010612150713/http://www.national.com/ds/DP/DP8390D.pdf
// =============================================================================
bitflags! {
    pub struct ReceiveStatusRegister : u8 {
        const RSR_PRX = 0x01; // Packet Received Intact
        const RSR_CRC = 0x02; // CRC Error
        const RSR_FAE = 0x04; // Frame Alignment Error
        const RSR_FO  = 0x08; // FIFO Overrun
        const RSR_MPA = 0x10; // Missed Packet
        const RSR_PHY = 0x20; // Physical/Multicast Address
        const RSR_DIS = 0x40; // Receiver Disabled
        const RSR_DFR = 0x80; // Deferring
    }
}
