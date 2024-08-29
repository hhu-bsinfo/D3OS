use alloc::boxed::Box;
use crate::device::serial::ComPort::{Com1, Com2, Com3, Com4};
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use stream::{InputStream, OutputStream};
use alloc::string::String;
use alloc::sync::Arc;
use core::ptr;
use bitflags::bitflags;
use log::info;
use nolock::queues::mpmc::bounded::scq::{Receiver, Sender};
use nolock::queues::{mpmc, DequeueError};
use spin::Mutex;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};
use crate::{apic, interrupt_dispatcher, scheduler};

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u16)]
pub enum ComPort {
    Com1 = 0x3f8,
    Com2 = 0x2f8,
    Com3 = 0x3e8,
    Com4 = 0x2e8,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
#[repr(u16)]
pub enum BaudRate {
    Baud115200 = 1,
    Baud57600 = 2,
    Baud38400 = 3,
    Baud28800 = 4,
    Baud23040 = 5,
    Baud19200 = 6,
    Baud14400 = 8,
    Baud12800 = 9,
    Baud11520 = 10,
    Baud9600 = 12,
    Baud7680 = 15,
    Baud7200 = 16,
    Baud6400 = 18,
    Baud5760 = 20,
    Baud4800 = 24,
    Baud4608 = 25,
    Baud3840 = 30,
    Baud3600 = 32,
    Baud3200 = 36,
    Baud2880 = 40,
    Baud2560 = 45,
    Baud2400 = 48,
    Baud2304 = 50,
    Baud1920 = 60,
    Baud1800 = 64,
    Baud1600 = 72,
    Baud1536 = 75,
    Baud1440 = 80,
    Baud1280 = 90,
    Baud1200 = 96,
    Baud1152 = 100,
    Baud960 = 120,
    Baud900 = 128,
    Baud800 = 144,
    Baud768 = 150,
    Baud720 = 160,
    Baud640 = 180,
    Baud600 = 192,
    Baud576 = 200,
    Baud512 = 225,
    Baud480 = 240,
    Baud450 = 256,
    Baud400 = 288,
    Baud384 = 300,
    Baud360 = 320,
    Baud320 = 360,
    Baud300 = 384,
    Baud288 = 400,
    Baud256 = 450,
    Baud240 = 480,
    Baud225 = 512,
    Baud200 = 576,
    Baud192 = 600,
    Baud180 = 640,
    Baud160 = 720,
    Baud150 = 768,
    Baud144 = 800,
    Baud128 = 900,
    Baud120 = 960,
    Baud100 = 1152,
    Baud96 = 1200,
    Baud90 = 1280,
    Baud80 = 1440,
    Baud75 = 1536,
    Baud72 = 1600,
    Baud64 = 1800,
    Baud60 = 1920,
    Baud50 = 2304,
    Baud48 = 2400,
    Baud45 = 2560,
    Baud40 = 2880,
    Baud36 = 3200,
    Baud32 = 3600,
    Baud30 = 3840,
    Baud25 = 4608,
    Baud24 = 4800,
    Baud20 = 5760,
    Baud18 = 6400,
    Baud16 = 7200,
    Baud15 = 7680,
    Baud12 = 9600,
    Baud10 = 11520,
    Baud9 = 12800,
    Baud8 = 14400,
    Baud6 = 19200,
    Baud5 = 23040,
    Baud4 = 28800,
    Baud3 = 38400,
    Baud2 = 57600,
}

bitflags! {
    struct LineStatus: u8 {
        const DATA_READY = 0x01;
        const OVERRUNG_ERROR = 0x02;
        const PARITOTY_ERROR = 0x04;
        const FRAMING_ERROR = 0x08;
        const BREAK_INDICATOR = 0x10;
        const TRANSMITTER_HOLDING_REGISTER_EMPTY = 0x20;
        const TRANSMITTER_EMPTY = 0x40;
        const IMPENDING_ERROR = 0x80;
    }
}

bitflags! {
    struct InterruptStatus: u8 {
        const InterruptPending = 0x01;
    }
}

pub struct SerialPort {
    port: ComPort,
    transceiver: Transceiver,
    interrupt_status: Mutex<PortReadOnly<u8>>,
    buffer: Option<(Receiver<u8>, Sender<u8>)>
}

struct Transceiver {
    port: ComPort,
    receive_buffer: Mutex<PortReadOnly<u8>>,
    transmit_buffer: Mutex<PortWriteOnly<u8>>,
    interrupt_control: Mutex<Port<u8>>,
    line_control: Mutex<Port<u8>>,
    line_status: PortReadOnly<u8>,
}

impl Transceiver {
    fn new(port: ComPort) -> Self {
        let base = port as u16;
        Self {
            port,
            receive_buffer: Mutex::new(PortReadOnly::new(base)),
            transmit_buffer: Mutex::new(PortWriteOnly::new(base)),
            interrupt_control: Mutex::new(Port::new(base + 1)),
            line_control: Mutex::new(Port::new(base + 3)),
            line_status: PortReadOnly::new(base + 5),
        }
    }

    fn speed(&self, speed: BaudRate) {
        let mut interrupt_control = self.interrupt_control.lock();
        let mut line_control = self.line_control.lock();
        let mut data = self.transmit_buffer.lock();

        info!("Setting baud rate of {:?} to {:?}", self.port, speed);

        unsafe  {
            let interrupt_backup = interrupt_control.read(); // Backup interrupt register
            let line_control_backup = line_control.read(); // Backup line control register

            line_control.write(0x80); // Enable DLAB, so that the divisor can be set

            data.write((speed as u16 & 0x00ff) as u8); // Divisor low byte
            interrupt_control.write(((speed as u16 & 0xff00) >> 8) as u8); // Divisor high byte

            line_control.write(line_control_backup); // Restore line control register
            interrupt_control.write(interrupt_backup); // Restore interrupt register
        }
    }

    fn interrupts(&self, enabled: bool) {
        let mut interrupt_control = self.interrupt_control.lock();
        unsafe { interrupt_control.write(if enabled { 0x01 } else { 0x00 }) };
    }

    fn line_status(&self) -> LineStatus {
        unsafe {
            // Reading line status is always safe. However, PortReadOnly::read() needs a mutable reference.
            let reg = ptr::from_ref(&self.line_status).cast_mut().as_mut().unwrap();
            LineStatus::from_bits_truncate(reg.read())
        }
    }

    fn readable(&self) -> bool {
        self.line_status().contains(LineStatus::DATA_READY)
    }

    fn writable(&self) -> bool {
        self.line_status().contains(LineStatus::TRANSMITTER_EMPTY)
    }

    fn read(&self) -> Option<u8> {
        match self.readable() {
            true => { Some(unsafe { self.receive_buffer.lock().read() }) }
            false => None,
        }
    }

    fn write(&self, byte: u8) {
        let mut buffer = self.transmit_buffer.lock();
        while !self.writable() {
            scheduler().switch_thread_no_interrupt();
        }

        unsafe { buffer.write(byte) };
    }
}

struct SerialInterruptHandler {
    serial_port: Arc<SerialPort>,
}

impl SerialInterruptHandler {
    pub const fn new(serial_port: Arc<SerialPort>) -> Self {
        Self { serial_port }
    }
}

pub fn check_port(port: ComPort) -> bool {
    let mut scratch = Port::<u8>::new(port as u16 + 7);

    (0..0xff).all(|i| {
        unsafe {
            scratch.write(i);
            scratch.read() == i
        }
    })
}

impl OutputStream for SerialPort {
    fn write_byte(&self, b: u8) {
        self.write_str(&String::from(char::from(b)));
    }

    fn write_str(&self, string: &str) {
        for b in string.bytes() {
            if b == '\n' as u8 {
                self.write_str("\r");
            }

            self.transceiver.write(b);
        }
    }
}

impl InputStream for SerialPort {
    fn read_byte(&self) -> i16 {
        loop {
            if let Some(buffer) = &self.buffer {
                match buffer.0.try_dequeue() {
                    Ok(byte) => return byte as i16,
                    Err(DequeueError::Closed) => return -1,
                    Err(_) => {}
                }
            } else {
                panic!("Serial: Trying to read before initialization!");
            }
        }
    }
}

impl InterruptHandler for SerialInterruptHandler {
    fn trigger(&self) {
        if self.serial_port.interrupt_status.is_locked() || self.serial_port.transceiver.receive_buffer.is_locked() {
            panic!("Serial: Required register is locked during interrupt!");
        }

        let interrupt_status = InterruptStatus::from_bits_truncate(unsafe { self.serial_port.interrupt_status.lock().read() });
        if !interrupt_status.contains(InterruptStatus::InterruptPending) {
            return;
        }

        let transceiver = &self.serial_port.transceiver;
        if let Some(buffer) = &self.serial_port.buffer {
            while let Some(data) = transceiver.read() {
                while buffer.1.try_enqueue(data).is_err() {
                    if buffer.0.try_dequeue().is_err() {
                        panic!("Serial: Failed to store received byte in buffer!");
                    }
                }
            }
        }
    }
}

impl SerialPort {
    pub fn new(port: ComPort, speed: BaudRate, buffer_cap: usize) -> Self {
        let transceiver = Transceiver::new(port);
        transceiver.interrupts(false);
        transceiver.speed(speed);

        Self {
            port,
            transceiver,
            interrupt_status: Mutex::new(PortReadOnly::new(port as u16 + 2)),
            buffer: Some(mpmc::bounded::scq::queue(buffer_cap))
        }
    }

    pub fn new_write_only(port: ComPort) -> Self {
        let transceiver = Transceiver::new(port);
        transceiver.interrupts(false);
        transceiver.speed(BaudRate::Baud115200);

        Self {
            port,
            transceiver,
            interrupt_status: Mutex::new(PortReadOnly::new(port as u16 + 2)),
            buffer: None
        }
    }

    pub fn plugin(serial_port: Arc<SerialPort>) {
        let vector = match serial_port.port {
            Com1 | Com3 => InterruptVector::Com1,
            Com2 | Com4 => InterruptVector::Com2,
        };

        serial_port.transceiver.interrupts(true);
        interrupt_dispatcher().assign(vector, Box::new(SerialInterruptHandler::new(serial_port)));
        apic().allow(vector);
    }
}
