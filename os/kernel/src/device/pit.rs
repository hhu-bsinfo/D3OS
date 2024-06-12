use crate::device::qemu_cfg;
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use alloc::boxed::Box;
use core::arch::asm;
use core::hint::spin_loop;
use spin::Mutex;
use x86_64::instructions::port::{Port, PortWriteOnly};
use crate::{apic, interrupt_dispatcher, timer};

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
        self.pending_incs += 1;

        if let Some(mut timer) = timer().try_write() {
            while self.pending_incs > 0 {
                timer.inc_systime();
                self.pending_incs -= 1;
            }
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
        interrupt_dispatcher().assign(InterruptVector::Pit, Box::new(TimerInterruptHandler::new()));
        apic().allow(InterruptVector::Pit);
    }

    pub fn systime_ms(&self) -> usize {
        return self.systime_ns / 1000000;
    }

    pub fn wait(ms: usize) {
        let end_time = timer().read().systime_ms() + ms;
        while timer().read().systime_ms() < end_time {
            spin_loop();
        }
    }

    fn inc_systime(&mut self) {
        self.systime_ns += self.interval_ns;
    }
}

/// Used to calibrate the APIC timer.
#[naked]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn early_delay_50ms() {
    asm!(
    "mov al, 0x30", // Channel 0, mode 0, low-/high byte access mode
    "outb 0x43", // Control port

    // Set counter value to 0xe90b (roughly 50ms)
    "mov al, 0x0b", // Low byte
    "outb 0x40", // Data port
    "mov al, 0xe9", // High byte
    "outb 0x40", // Data port

    // Wait until output pin bit is set (counter reached 0)
    "2:", // Loop label
    "mov al, 0xe2", // Read status byte -> channel 0, mode 0, low-/high byte access mode
    "outb 0x43", // Control port
    "inb 0x40", // Data port
    "test al, 0x80", // Test bit 7 (output pin state)
    "jz 2b", // If bit 7 is not set -> loop

    "ret",
    options(noreturn)
    );
}