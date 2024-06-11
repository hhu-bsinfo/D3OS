/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: log                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Logger implementation. Support one or several output streams.   ║
   ║         Messages are dumped on each output stream.                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, HHU                                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use crate::device::serial;
use crate::device::serial::ComPort;
use crate::device::serial::SerialPort;
use crate::{allocator, logger, timer};
use graphic::ansi;
use stream::OutputStream;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr;
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use crate::built_info;

pub struct Logger {
    level: Level,
    streams: Vec<Box<&'static dyn OutputStream>>,
    serial: Option<SerialPort>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        return metadata.level() <= self.level;
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level = record.metadata().level();
        let file = record.file().unwrap_or("unknown").split('/').rev().next().unwrap_or("unknown");
        let line = record.line().unwrap_or(0);

        let mut logger = logger().lock();
        if logger.streams.is_empty() {
            if let Some(serial) = logger.serial.as_mut() {
                serial.write_str(ansi::FOREGROUND_CYAN);
                serial.write_str("[0.000]");
                serial.write_str(ansi_color(level));
                serial.write_str("[");
                serial.write_str(level_token(level));
                serial.write_str("]");
                serial.write_str(ansi::FOREGROUND_MAGENTA);
                serial.write_str("[");
                serial.write_str(file);
                serial.write_str("] ");
                serial.write_str(ansi::FOREGROUND_DEFAULT);

                if allocator().is_initialized() {
                    serial.write_str(record.args().to_string().as_str());
                } else {
                    serial.write_str(record.args().as_str().unwrap_or_else(|| { "Formatted messages are not supported before heap initialization!" }));
                }

                serial.write_str("\n");
            }
        } else {
            let systime = timer().read().systime_ms();
            let seconds = systime / 1000;
            let fraction = systime % 1000;

            let string = format!("{}[{}.{:0>3}]{}[{}]{}[{}@{:0>3}]{} {}\n", ansi::FOREGROUND_CYAN, seconds, fraction,
                                 ansi_color(level), level_token(level),ansi::FOREGROUND_MAGENTA, file, line, ansi::FOREGROUND_DEFAULT, record.args());

            for i in 0..self.streams.len() {
                logger.streams[i].write_str(&string);
            }
        }
    }

    fn flush(&self) {}
}

impl Logger {
    pub const fn new() -> Self {
        Self {
            level: Level::Info,
            streams: Vec::new(),
            serial: None,
        }
    }

    pub fn init(&self) -> Result<(), SetLoggerError> {
        unsafe {
            logger().force_unlock();
        } // The caller needed to call logger().lock() in order to call init()
        let mut logger = logger().lock();

        if serial::check_port(ComPort::Com1) {
            logger.serial = Some(SerialPort::new(ComPort::Com1))
        } else if serial::check_port(ComPort::Com2) {
            logger.serial = Some(SerialPort::new(ComPort::Com2))
        } else if serial::check_port(ComPort::Com3) {
            logger.serial = Some(SerialPort::new(ComPort::Com3))
        } else if serial::check_port(ComPort::Com4) {
            logger.serial = Some(SerialPort::new(ComPort::Com4))
        }

        if logger.serial.is_some() {
            logger.serial.as_mut().unwrap().init_write_only();
        }

        if built_info::PROFILE == "debug" {
            logger.level = Level::Debug;
        }

        unsafe {
            let logger_ref = ptr::from_ref(logger.deref()).as_ref().unwrap();
            return log::set_logger(logger_ref).map(|()| log::set_max_level(LevelFilter::Debug));
        }
    }

    pub fn register(&mut self, stream: &'static dyn OutputStream) {
        self.streams.push(Box::new(stream));
    }

    pub fn remove(&mut self, stream: &dyn OutputStream) {
        self.streams.retain(|element| {
            !ptr::addr_eq(ptr::from_ref(*element.as_ref()), ptr::from_ref(stream))
        });
    }
}

fn ansi_color(level: Level) -> &'static str {
    match level {
        Level::Trace => ansi::FOREGROUND_BRIGHT_WHITE,
        Level::Debug => ansi::FOREGROUND_BRIGHT_GREEN,
        Level::Info => ansi::FOREGROUND_BRIGHT_BLUE,
        Level::Warn => ansi::FOREGROUND_BRIGHT_YELLOW,
        Level::Error => ansi::FOREGROUND_BRIGHT_RED,
    }
}

fn level_token(level: Level) -> &'static str {
    match level {
        Level::Trace => "TRC",
        Level::Debug => "DBG",
        Level::Info => "INF",
        Level::Warn => "WRN",
        Level::Error => "ERR",
    }
}
