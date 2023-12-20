use alloc::boxed::Box;
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr;
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use crate::device::serial;
use crate::device::serial::ComPort;
use crate::device::serial::SerialPort;
use crate::kernel;
use crate::library::graphic::ansi;
use crate::library::io::stream::OutputStream;

pub struct Logger {
    level: Level,
    streams: Vec<Box<&'static mut dyn OutputStream>>,
    serial: Option<SerialPort>
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        return metadata.level() <= self.level;
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let ms = kernel::get_time_service().get_systime_ms();
        let seconds = ms / 1000;
        let fraction = ms % 1000;
        let level = record.metadata().level();
        let file = record.file().unwrap_or("unknown")
            .split('/').rev().next().unwrap_or("unknown");
        let line = record.line().unwrap_or(0);

        let mut logger = kernel::get_logger().lock();
        if logger.streams.is_empty() {
            if let Some(serial) = logger.serial.as_mut() {
                serial.write_str(ansi::FOREGROUND_CYAN);
                serial.write_str("[0.000]");
                serial.write_str(ansi_color(level));
                serial.write_str("[");
                serial.write_str(level.as_str());
                serial.write_str("]");
                serial.write_str(ansi::FOREGROUND_DEFAULT);
                serial.write_str("[");
                serial.write_str(file);
                serial.write_str("] ");

                if kernel::get_memory_service().is_initialized() {
                    serial.write_str(record.args().to_string().as_str());
                } else {
                    serial.write_str(record.args().as_str().unwrap_or_else(|| "Formatted messages are not supported before heap initialization!"));
                }

                serial.write_str("\n");
            }
        } else {
            let string = format!("{}[{}.{:0>3}]{}[{}]{}[{}@{:0>3}] {}\n", ansi::FOREGROUND_CYAN, seconds, fraction, ansi_color(level), level.as_str(), ansi::FOREGROUND_DEFAULT, file, line, record.args());

            for i in 0..self.streams.len() {
                let stream = logger.streams[i].as_mut();
                stream.write_str(&string);
            }
        }
    }

    fn flush(&self) {}
}

impl Logger {
    pub const fn new() -> Self {
        Self { level: Level::Info, streams: Vec::new(), serial: None }
    }

    pub fn init(&self) -> Result<(), SetLoggerError> {
        unsafe { kernel::get_logger().force_unlock(); }
        let mut logger = kernel::get_logger().lock();

        if serial::check_port(ComPort::Com1) { logger.serial = Some(SerialPort::new(ComPort::Com1)) }
        else if serial::check_port(ComPort::Com2) { logger.serial = Some(SerialPort::new(ComPort::Com2)) }
        else if serial::check_port(ComPort::Com3) { logger.serial = Some(SerialPort::new(ComPort::Com3)) }
        else if serial::check_port(ComPort::Com4) { logger.serial = Some(SerialPort::new(ComPort::Com4)) }

        if logger.serial.is_some() {
            logger.serial.as_mut().unwrap().init_write_only();
        }

        unsafe {
            let logger_ref = ptr::from_ref(logger.deref()).as_ref().unwrap();
            return log::set_logger(logger_ref).map(|()| log::set_max_level(LevelFilter::Info));
        }
    }

    pub fn register(&mut self, stream: &'static mut dyn OutputStream) {
        self.streams.push(Box::new(stream));
    }

    pub fn remove(&mut self, stream: &mut dyn OutputStream) {
        self.streams.retain(|element| !ptr::addr_eq(ptr::from_ref(*element.as_ref()), ptr::from_ref(stream)));
    }
}

fn ansi_color(level: Level) -> &'static str {
    match level {
        Level::Trace => ansi::FOREGROUND_BRIGHT_WHITE,
        Level::Debug => ansi::FOREGROUND_BRIGHT_GREEN,
        Level::Info => ansi::FOREGROUND_BRIGHT_BLUE,
        Level::Warn => ansi::FOREGROUND_BRIGHT_YELLOW,
        Level::Error => ansi::FOREGROUND_BRIGHT_RED
    }
}