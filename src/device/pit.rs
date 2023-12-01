use alloc::boxed::Box;
use alloc::format;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::port::{Port, PortWriteOnly};
use crate::device::qemu_cfg;
use crate::kernel;
use crate::kernel::interrupt_dispatcher::InterruptVector;
use crate::kernel::isr::ISR;
use crate::kernel::log::Logger;

lazy_static!{
    static ref LOG: Logger = Logger::new("PIT");
}

pub const BASE_FREQUENCY: usize = 1193182;

pub struct Pit {
    ctrl_port: Mutex<PortWriteOnly<u8>>,
    data_port: Mutex<Port<u8>>,
    interval_ns: usize,
}

pub struct PitISR {
    interval_ns: usize
}

impl ISR for PitISR {
    fn trigger(&self) {
        let time_service = kernel::get_time_service();
        time_service.inc_systime(self.interval_ns);

        if time_service.get_systime_ms() % 10 == 0 {
            kernel::get_thread_service().switch_thread();
        }
    }
}

impl PitISR {
    pub const fn new(interval_ns: usize) -> Self {
        Self { interval_ns }
    }
}

impl Pit {
    pub const fn new() -> Self {
        Self { ctrl_port: Mutex::new(PortWriteOnly::new(0x43)), data_port: Mutex::new(Port::new(0x40)), interval_ns: 0 }
    }

    pub fn set_int_rate(&mut self, interval_ms: usize) {
        let mut divisor = (BASE_FREQUENCY / 1000) * interval_ms;
        if divisor > u16::MAX as usize {
            divisor = u16::MAX as usize;
        }

        self.interval_ns = 1000000000 / (BASE_FREQUENCY / divisor);

        LOG.info(format!("Setting timer interval to [{}ms] (Divisor: [{}])", if self.interval_ns / 1000000 < 1 { 1 } else { self.interval_ns / 1000000 }, divisor).as_str());

        // For some reason, the PIT interrupt rate is doubled, when it is attached to an IO APIC (only in QEMU)
        if qemu_cfg::is_available() {
            divisor *= 2;
        }

        let mut ctrl_port = self.ctrl_port.lock();
        let mut data_port = self.data_port.lock();

        unsafe {
            ctrl_port.write(0x36); // Select channel 0, Use low-/high byte access mode, Set operating mode to rate generator
            data_port.write((divisor & 0xff) as u8); // Low byte
            data_port.write(((divisor >> 8) & 0xff) as u8); // High byte
        }
    }

    pub fn plugin(&self) {
        let int_service = kernel::get_interrupt_service();
        int_service.assign_handler(InterruptVector::Pit, Box::new(PitISR::new(self.interval_ns)));
        int_service.allow_interrupt(InterruptVector::Pit);
    }
}