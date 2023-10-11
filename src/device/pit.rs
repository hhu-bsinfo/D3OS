use alloc::boxed::Box;
use x86_64::instructions::port::{Port, PortWriteOnly};
use crate::device::pic;
use crate::kernel::int_disp;
use crate::kernel::int_disp::InterruptVector;
use crate::kernel::isr::ISR;

pub const BASE_FREQUENCY: usize = 1193182;

static mut TIMER: Option<Pit> = None;

struct Pit {
    ctrl_port: PortWriteOnly<u8>,
    data_port: Port<u8>,

    interval_ns: usize,
    elapsed_time_ns: usize
}

#[derive(Default)]
struct PitISR;

impl ISR for PitISR {
    fn trigger(&self) {
        unsafe {
            if let Some(pit) = TIMER.as_mut() {
                pit.elapsed_time_ns += pit.interval_ns;
            }
        }
    }
}

pub fn init() {
    unsafe {
        if TIMER.is_none() {
            TIMER = Some(Pit::new(1));
        }
    }
}

pub fn plugin() {
    int_disp::assign(InterruptVector::Pit, Box::new(PitISR::default()));
    pic::allow(InterruptVector::Pit);
}

pub fn get_systime_ms() -> usize {
    unsafe {
        return match TIMER.as_ref() {
            Some(pit) => pit.get_systime_ms(),
            None => 0
        };
    }
}

pub fn wait(ms: usize) {
    unsafe {
        if let Some(pit) = TIMER.as_ref() {
            let end_time = pit.get_systime_ms() + ms;
            while pit.get_systime_ms() < end_time {}
        }
    }
}

impl Pit {
    fn new(interval_ms: usize) -> Self {
        let mut pit = Self { ctrl_port: PortWriteOnly::new(0x43), data_port: Port::new(0x40), interval_ns: 0, elapsed_time_ns: 0 };
        pit.set_int_rate(interval_ms);

        return pit;
    }

    fn set_int_rate(&mut self, interval_ms: usize) {
        let mut divisor = (BASE_FREQUENCY / 1000) * interval_ms;
        if divisor > u16::MAX as usize {
            divisor = u16::MAX as usize;
        }

        self.interval_ns = 1000000000 / (BASE_FREQUENCY / divisor);

        unsafe {
            self.ctrl_port.write(0x36); // Select channel 0, Use low-/high byte access mode, Set operating mode to rate generator
            self.data_port.write((divisor & 0xff) as u8); // Low byte
            self.data_port.write(((divisor >> 8) & 0xff) as u8); // High byte
        }
    }

    fn get_systime_ms(&self) -> usize {
        self.elapsed_time_ns / 1000000
    }
}