use alloc::boxed::Box;
use alloc::sync::Arc;
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;
use x86_64::instructions::port::{Port, PortWriteOnly};
use crate::{apic, interrupt_dispatcher};

pub const BASE_FREQUENCY: usize = 1193182;
const NANOSECONDS_PER_TICK: usize = 1000000000 / BASE_FREQUENCY;

#[derive(Copy, Clone)]
#[allow(dead_code)]
#[repr(u8)]
enum BcdBinaryMode {
    Binary = 0x00,
    Bcd = 0x01
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
#[repr(u8)]
enum OperatingMode {
    InterruptOnTerminalCount = 0x00,
    OneShot = 0x01,
    RateGenerator = 0x02,
    SquareWaveGenerator = 0x03,
    SoftwareTriggeredStrobe = 0x04,
    HardwareTriggeredStrobe = 0x05,
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
#[repr(u8)]
enum AccessMode {
    LatchCount = 0x00,
    LowByteOnly = 0x01,
    HighByteOnly = 0x02,
    LowByteHighByte = 0x03
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
#[repr(u8)]
enum Channel  {
    Channel0 = 0x00,
    Channel1 = 0x01,
    Channel2 = 0x02,
    ReadBack = 0x03
}

pub struct Timer {
    registers: Mutex<Registers>,
    interval_ns: usize,
    systime_ns: AtomicUsize,
}

struct Registers {
    ctrl_port: PortWriteOnly<u8>,
    data_port: Port<u8>
}

struct Command {
    bcd_binary_mode: BcdBinaryMode,
    operating_mode: OperatingMode,
    access_mode: AccessMode,
    channel: Channel
}

struct TimerInterruptHandler {
    timer: Arc<Timer>,
}

impl Command {
    pub const fn new(operating_mode: OperatingMode, access_mode: AccessMode) -> Self {
        Self {
            bcd_binary_mode: BcdBinaryMode::Binary,
            operating_mode,
            access_mode,
            channel: Channel::Channel0
        }
    }

    pub fn as_u8(&self) -> u8 {
        (self.bcd_binary_mode as u8) |
        (self.operating_mode as u8) << 1 |
        (self.access_mode as u8) << 4 |
        (self.channel as u8) << 6
    }
}

impl InterruptHandler for TimerInterruptHandler {
    fn trigger(&self) {
        self.timer.inc_systime();
    }
}

impl TimerInterruptHandler {
    pub const fn new(timer: Arc<Timer>) -> Self {
        Self { timer }
    }
}

impl Registers {
    pub const fn new() -> Self {
        Self {
            ctrl_port: PortWriteOnly::new(0x43),
            data_port: Port::new(0x40)
        }
    }
}

impl Timer {
    pub fn new() -> Self {
        let mut timer = Self {
            registers: Mutex::new(Registers::new()),
            interval_ns: 0,
            systime_ns: AtomicUsize::new(0)
        };

        timer.interrupt_rate(1);
        timer
    }

    fn interrupt_rate(&mut self, interval_ms: usize) {
        let mut divisor = (interval_ms * 1000000) / NANOSECONDS_PER_TICK;
        if divisor > u16::MAX as usize {
            divisor = u16::MAX as usize;
        }

        self.interval_ns = NANOSECONDS_PER_TICK * divisor;

        let command = Command::new(OperatingMode::RateGenerator, AccessMode::LowByteHighByte);
        let mut registers = self.registers.lock();

        unsafe {
            registers.ctrl_port.write(command.as_u8());
            registers.data_port.write((divisor & 0xff) as u8); // Low byte
            registers.data_port.write((divisor >> 8) as u8); // High byte
        }
    }

    fn read_timer(&self) -> u16 {
        let mut registers = self.registers.lock();
        let mut timer: u16 = 0;
        let command = Command::new(OperatingMode::InterruptOnTerminalCount, AccessMode::LatchCount);

        unsafe {
            registers.ctrl_port.write(command.as_u8()); // Latch counter value
            timer |= registers.data_port.read() as u16; // Low byte
            timer |= (registers.data_port.read() as u16) << 8; // High byte
        }

        timer
    }

    pub fn plugin(timer: Arc<Timer>) {
        interrupt_dispatcher().assign(InterruptVector::Pit, Box::new(TimerInterruptHandler::new(timer)));
        apic().allow(InterruptVector::Pit);
    }

    pub fn systime_ms(&self) -> usize {
        self.systime_ns.load(Ordering::Relaxed) / 1000000
    }

    pub fn wait(&self, wait_time_ms: usize) {
        let wait_time_ns = wait_time_ms * 1000000;
        let mut elapsed_time_ns = 0;
        let mut last_timer_value = self.read_timer();

        while elapsed_time_ns < wait_time_ns {
            let timer_value = self.read_timer();
            let ticks = if last_timer_value >= timer_value {
                // Timer did not wrap around
                last_timer_value - timer_value
            } else {
                // Timer wrapped around
                (self.interval_ns / NANOSECONDS_PER_TICK) as u16 - timer_value + last_timer_value
            };

            elapsed_time_ns += ticks as usize * NANOSECONDS_PER_TICK;
            last_timer_value = timer_value;
        }
    }

    fn inc_systime(&self) {
        self.systime_ns.fetch_add(self.interval_ns, Ordering::Relaxed);
    }
}