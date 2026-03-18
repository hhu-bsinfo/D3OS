use spin::lock_api::Mutex;
use crate::device::pit;
use x86_64::instructions::port::{Port, PortWriteOnly};
use crate::timer;

pub struct Speaker {
    registers: Mutex<Registers>
}

struct Registers {
    ctrl_port: PortWriteOnly<u8>,
    data_port_2: PortWriteOnly<u8>,
    ppi_port: Port<u8>,
}

impl Registers {
    pub const fn new() -> Self {
        Self {
            ctrl_port: PortWriteOnly::new(0x43),
            data_port_2: PortWriteOnly::new(0x42),
            ppi_port: Port::new(0x61),
        }
    }
}

impl Speaker {
    pub const fn new() -> Self {
        Self { registers: Mutex::new(Registers::new()) }
    }

    pub fn on(&self, freq: usize) {
        let mut registers = self.registers.lock();
        let counter = pit::BASE_FREQUENCY / freq;

        unsafe {
            // Config counter
            registers.ctrl_port.write(0xb6);
            registers.data_port_2.write((counter % 256) as u8);
            registers.data_port_2.write((counter / 256) as u8);

            // Turn speaker on
            let status = registers.ppi_port.read();
            registers.ppi_port.write(status | 0x03);
        }
    }

    pub fn off(&self) {
        let mut registers = self.registers.lock();

        unsafe {
            let status = registers.ppi_port.read();
            registers.ppi_port.write(status & 0xfc);
        }
    }

    pub fn play(&self, freq: usize, duration_ms: usize) {
        let timer = timer();

        self.on(freq);
        timer.wait(duration_ms);
        self.off();
    }
}
