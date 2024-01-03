use crate::device::qemu_cfg;
use crate::kernel;
use crate::kernel::interrupt::interrupt_dispatcher::InterruptVector;
use crate::kernel::interrupt::interrupt_handler::InterruptHandler;
use alloc::boxed::Box;
use core::hint::spin_loop;
use spin::Mutex;
use x86_64::instructions::port::{Port, PortWriteOnly};

pub const BASE_FREQUENCY: usize = 1193182;

pub struct Timer {
    ctrl_port: Mutex<PortWriteOnly<u8>>,
    data_port: Mutex<Port<u8>>,
    interval_ns: usize,
    systime_ns: usize,
}

struct TimerInterruptHandler {
    pending_incs: usize,
}

impl InterruptHandler for TimerInterruptHandler {
    fn trigger(&mut self) {
        let mut systime = 1;
        self.pending_incs += 1;
        if let Some(mut timer) = kernel::timer().try_write() {
            while self.pending_incs > 0 {
                timer.inc_systime();
                self.pending_incs -= 1;
            }

            systime = timer.systime_ms();
        }

        if systime % 10 == 0 {
            kernel::scheduler().switch_thread();
        }
    }
}

impl TimerInterruptHandler {
    pub const fn new() -> Self {
        Self { pending_incs: 0 }
    }
}

impl Timer {
    pub const fn new() -> Self {
        Self {
            ctrl_port: Mutex::new(PortWriteOnly::new(0x43)),
            data_port: Mutex::new(Port::new(0x40)),
            interval_ns: 0,
            systime_ns: 0,
        }
    }

    pub fn interrupt_rate(&mut self, interval_ms: usize) {
        let mut divisor = (BASE_FREQUENCY / 1000) * interval_ms;
        if divisor > u16::MAX as usize {
            divisor = u16::MAX as usize;
        }

        self.interval_ns = 1000000000 / (BASE_FREQUENCY / divisor);

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
        kernel::interrupt_dispatcher()
            .assign(InterruptVector::Pit, Box::new(TimerInterruptHandler::new()));
        kernel::apic().allow(InterruptVector::Pit);
    }

    pub fn systime_ms(&self) -> usize {
        return self.systime_ns / 1000000;
    }

    pub fn wait(ms: usize) {
        let end_time = kernel::timer().read().systime_ms() + ms;
        while kernel::timer().read().systime_ms() < end_time {
            spin_loop();
        }
    }

    fn inc_systime(&mut self) {
        self.systime_ns += self.interval_ns;
    }
}
