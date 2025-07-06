use spin::{Mutex, RwLock};
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

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
    pub fn new(base_address: u16) -> Self {
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
