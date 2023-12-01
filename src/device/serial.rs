use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use lazy_static::lazy_static;
use nolock::queues::{DequeueError, mpmc};
use nolock::queues::mpmc::bounded::scq::{Receiver, Sender};
use x86_64::instructions::port::Port;
use crate::device::serial::ComPort::{Com1, Com2, Com3, Com4};
use crate::kernel;
use crate::kernel::interrupt_dispatcher::InterruptVector;
use crate::kernel::isr::ISR;
use crate::kernel::log::Logger;
use crate::library::io::stream::{InputStream, OutputStream};

lazy_static! {
static ref LOG: Logger = Logger::new("COM");
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u16)]
pub enum ComPort {
    Com1 = 0x3f8,
    Com2 = 0x2f8,
    Com3 = 0x3e8,
    Com4 = 0x2e8
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
    Baud2 = 57600
}

pub struct SerialPort {
    port: ComPort,
    data_reg: Port<u8>,
    interrupt_reg: Port<u8>,
    fifo_control_reg: Port<u8>,
    line_control_reg: Port<u8>,
    modem_control_reg: Port<u8>,
    line_status_reg: Port<u8>,

    buffer: Option<(Receiver<u8>, Sender<u8>)>
}

#[derive(Default)]
pub struct SerialISR;

pub unsafe fn check_port(port: ComPort) -> bool {
    let mut scratch = Port::<u8>::new(port as u16 + 7);

    for i in 0 .. 0xff {
        scratch.write(i as u8);
        if scratch.read() != i {
            return false;
        }
    }

    return true;
}

impl OutputStream for SerialPort {
    fn write_byte(&mut self, b: u8) {
        self.write_str(&String::from(char::from(b)));
    }

    fn write_str(&mut self, string: &str) {
        if self.buffer.is_none() {
            panic!("Serial: Trying to write before initialization!");
        }

        for b in string.bytes() {
            if b == '\n' as u8 {
                self.write_byte(0x0d);
            }

            unsafe {
                while (self.line_status_reg.read() & 0x20) != 0x20 {
                    core::hint::spin_loop();
                }

                self.data_reg.write(b);
            }
        }
    }
}

impl InputStream for SerialPort {
    fn read_byte(&mut self) -> i16 {
        loop {
            if let Some(buffer) = self.buffer.as_mut() {
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

impl ISR for SerialISR {
    fn trigger(&self) {
        let serial = match kernel::get_device_service().get_serial_port() {
            Some(serial) => serial,
            None => return
        };

        unsafe {
            if (serial.fifo_control_reg.read() & 0x01) == 0x01 {
                return;
            }

            while (serial.line_status_reg.read() & 0x01) == 0x01 {
                let byte = serial.data_reg.read();
                match serial.buffer.as_mut() {
                    Some(buffer) => {
                        while buffer.1.try_enqueue(byte).is_err() {
                            if buffer.0.try_dequeue().is_err() {
                                panic!("Serial: Failed to store received byte in buffer!");
                            }
                        }
                    }
                    None => panic!("Serial: ISR called before initialization!")
                }
            }
        }
    }
}

impl SerialPort {
    pub const fn new(port: ComPort) -> Self {
        Self {
            port,
            data_reg: Port::<u8>::new(port as u16),
            interrupt_reg: Port::<u8>::new(port as u16 + 1),
            fifo_control_reg: Port::<u8>::new(port as u16 + 2),
            line_control_reg: Port::<u8>::new(port as u16 + 3),
            modem_control_reg: Port::<u8>::new(port as u16 + 4),
            line_status_reg: Port::<u8>::new(port as u16 + 5),
            buffer: None
        }
    }

    pub unsafe fn init(&mut self, buffer_cap: usize, speed: BaudRate) {
        self.buffer = Some(mpmc::bounded::scq::queue(buffer_cap));

        self.interrupt_reg.write(0x00); // Disable all interrupts
        self.line_control_reg.write(0x80); // Enable DLAB, so that the divisor can be set

        self.set_speed(speed);

        self.line_control_reg.write(0x03); // 8 bits per char, no parity, one stop bit
        self.fifo_control_reg.write(0x07); // Enable FIFO-buffers, Clear FIFO-buffers, Trigger interrupt after each byte
        self.modem_control_reg.write(0x0b); // Enable data lines
    }

    pub fn set_speed(&mut self, speed: BaudRate) {
        if self.buffer.is_none() {
            panic!("Serial: Trying to set speed before initialization!");
        }

        LOG.info(format!("Setting speed to [{:?}]", speed).as_str());

        unsafe {
            let interrupt_backup = self.interrupt_reg.read();
            let line_control_backup = self.line_control_reg.read();

            self.interrupt_reg.write(0x00); // Disable all interrupts
            self.line_control_reg.write(0x80); // Enable DLAB, so that the divisor can be set

            self.data_reg.write((speed as u16 & 0x00ff) as u8); // Divisor low byte
            self.interrupt_reg.write(((speed as u16 & 0xff00) >> 8) as u8); // Divisor high byte

            self.line_control_reg.write(line_control_backup); // Restore line control register
            self.interrupt_reg.write(interrupt_backup); // Restore interrupt register
        }
    }

    pub fn plugin(&mut self) {
        let vector = match self.port {
            Com1 | Com3 => InterruptVector::Com1,
            Com2 | Com4 => InterruptVector::Com2
        };

        let int_service = kernel::get_interrupt_service();
        int_service.assign_handler(vector, Box::new(SerialISR::default()));
        int_service.allow_interrupt(vector);

        unsafe { self.interrupt_reg.write(0x01) } // Enable interrupts
    }
}

