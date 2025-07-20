// =============================================================================
// FILE        : register_flags.rs
// AUTHOR      : Johann Spenrath
// DESCRIPTION : defines the registers of the NE2000 and their corresponding bits,
//               which control the Ne2000's functionalities
//               use these Constants for setting the correct bits in a register
//
// TODO: Add comment with reference for each Register
//
// NOTES:
//
//
// =============================================================================
// DEPENDENCIES:
// =============================================================================
use bitflags::bitflags;
// =============================================================================
bitflags! {
    pub struct PageRegisters : u8 {
        const COMMAND     = 0x00;         // R|W COMMAND used for P0, P1, P2
        // P0 Write Registers
        const P0_PSTART   = 0x01;        // W Page Start Register
        const P0_PSTOP    = 0x02;        // W Page Stop Register
        const P0_BNRY     = 0x03;        // R|W Boundary Pointer  P0
        const P0_TPSR     = 0x04;        // W Transmit Page Start Address
        const P0_TBCR0    = 0x05;        // W Transmit Byte Count Register 0
        const P0_TBCR1    = 0x06;        // W Transmit Byte Count Register 1
        const P0_ISR      = 0x07;        // R|W Interrupt Status Register P0
        const P0_RSAR0    = 0x08;        // W Remote Start Address Register 0
        const P0_RSAR1    = 0x09;        // W Remote Start Address Register 1
        const P0_RBCR0    = 0x0A;        // W Remote Byte Count Register 0
        const P0_RBCR1    = 0x0B;        // W Remote Byte Count Register 1
        const P0_RCR      = 0x0C;        // W Receive Configuration Register
        const P0_TCR      = 0x0D;        // W Transmit Configuration Register
        const P0_DCR      = 0x0E;        // W Data Configuration Register
        const P0_IMR      = 0x0F;        // W Interrupt Mask Register
        // P0 Read Registers
        const P0_CLDA0    = 0x01;        // R Current Local DMA Address 0
        const P0_CLDA1    = 0x02;        // R Current Local DMA Address 1
        const P0_TSR      = 0x04;        // R Transmit Status Register
        const P0_NCR      = 0x05;        // R Number of Collisions Register
        const P0_FIFO     = 0x06;        // R FIFO */
        const P0_CRDA0    = 0x08;        // R Current Remote DMA Address 0
        const P0_CRDA1    = 0x09;        // R Current Remote DMA Address 1
        const P0_RSR      = 0x0C;        // R Receive Status Register
        const P0_CNTR0    = 0x0D;        // R Tally Counter 0 (Frame Alignment Errors)
        const P0_CNTR1    = 0x0E;        // R Tally Counter 1 (CRC Errors)
        const P0_CNTR2    = 0x0F;        // R Tally Counter 2 (Missed Packet Error)
        // P1 Read and Write Registers
        const P1_PAR0     = 0x01;        //* R|W Physical Address Register 0
        const P1_PAR1     = 0x02;        //* R|W Physical Address Register 1
        const P1_PAR2     = 0x03;        //* R|W Physical Address Register 2
        const P1_PAR3     = 0x04;        //* R|W Physical Address Register 3
        const P1_PAR4     = 0x05;        //* R|W Physical Address Register 4
        const P1_PAR5     = 0x06;        //* R|W Physical Address Register 5
        const P1_CURR     = 0x07;        //* R|W Current Page Register */
        const P1_MAR0     = 0x08;        //* R|W Multicast Address Register 0
        const P1_MAR1     = 0x09;        //* R|W Multicast Address Register 1
        const P1_MAR2     = 0x0A;        //* R|W Multicast Address Register 2
        const P1_MAR3     = 0x0B;        //* R|W Multicast Address Register 3
        const P1_MAR4     = 0x0C;        //* R|W Multicast Address Register 4 */
        const P1_MAR5     = 0x0D;        //* R|W Multicast Address Register 5 */
        const P1_MAR6     = 0x0E;        //* R|W Multicast Address Register 6 */
        const P1_MAR7     = 0x0F;        //* R|W Multicast Address Register 7 */
        // P2 Registers are only for diagnostic purposes.
        // P2 Write Registers
        const P2_CLDA0    = 0x01;        //* W Current Local DMA Address 0 */
        const P2_CLDA1    = 0x02;        //* W Current Local DMA Address 1 */
        const P2_RNPP     = 0x03;        //* R|W Remote Next Packet Pointer */
        const P2_LNPP     = 0x05;        //* R|W Local Next Packet Pointer */
        const P2_UPPER    = 0x06;        //* R|W Address Counter (Upper) */
        const P2_LOWER    = 0x07;        //* R|W Address Counter (Lower) */
        // P2 Read Registers
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
        const STA = 0x02; // START
        const TXP = 0x04; // Transmit Packet */
        const RD_0 = 0x08; // Remote DMA Command 0 */
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

bitflags! {

    // enable / disable interrupts
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
bitflags! {
    pub struct TransmitConfigurationRegister : u8 {
        const TCR_CRC  = 0x01;  //Inhibit CRC
        const TCR_LB0  = 0x02;  //Encoded Loop-back Control
        const TCR_LB1  = 0x04;  //Encoded Loop-back Control
        const TCR_ATD  = 0x08;  //Auto Transmit Disable
        const TCR_OFST = 0x10;  //Collision Offset Enable
    }
}

// Transmit Status Register as defined in DP8390D
// P. 24https://datasheetspdf.com/pdf-file/549771/NationalSemiconductor/DP8390D/1
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

// Receive Configuration Register as defined in DP8390D
// P.25 https://datasheetspdf.com/pdf-file/549771/NationalSemiconductor/DP8390D/1

bitflags! {

    pub struct ReceiveConfigurationRegister : u8 {
        const RCR_SEP = 0x01; // Save Error Packets
        const RCR_AR  = 0x02; // Accept Runt Packets
        const RCR_AB  = 0x04; // Accept Broadcast
        const RCR_AM  = 0x08; // Accept Multicast
        const RCR_PRO = 0x10; // Promiscuous Physical
        const RCR_MON = 0x20; // Monitor Mode
    }
}

// Receive Status Register as defined in DP8390D
// P.26 https://datasheetspdf.com/pdf-file/549771/NationalSemiconductor/DP8390D/1

bitflags! {
    pub struct ReceiveStatusRegister : u8 {
        const RSR_PRX = 0x01; //** Packet Received Intact
        const RSR_CRC = 0x02; //** CRC Error
        const RSR_FAE = 0x04; //** Frame Alignment Error
        const RSR_FO  = 0x08; //** FIFO Overrun
        const RSR_MPA = 0x10; //** Missed Packet
        const RSR_PHY = 0x20; //** Physical/Multicast Address
        const RSR_DIS = 0x40; //** Receiver Disabled
        const RSR_DFR = 0x80;  //** Deferring
    }
}
