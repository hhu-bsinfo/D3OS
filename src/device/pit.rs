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
    elapsed_time_ns: usize,
}

#[derive(Default)]
pub struct PitISR;

impl ISR for PitISR {
    fn trigger(&self) {
        let timer = kernel::get_device_service().get_timer();
        timer.elapsed_time_ns += timer.interval_ns;

        if timer.get_systime_ms() % 10 == 0 {
            kernel::get_thread_service().get_scheduler().switch_thread();
        }
    }
}

impl Pit {
    pub const fn new() -> Self {
        Self { ctrl_port: Mutex::new(PortWriteOnly::new(0x43)), data_port: Mutex::new(Port::new(0x40)), interval_ns: 0, elapsed_time_ns: 0 }
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

    pub fn get_systime_ms(&self) -> usize {
        self.elapsed_time_ns / 1000000
    }

    pub fn wait(&self, ms: usize) {
        let end_time = self.get_systime_ms() + ms;
        while self.get_systime_ms() < end_time {}
    }

    pub fn plugin(&self) {
        let int_service = kernel::get_interrupt_service();
        int_service.get_dispatcher().assign(InterruptVector::Pit, Box::new(PitISR::default()));
        int_service.get_apic().allow(InterruptVector::Pit);
    }
}