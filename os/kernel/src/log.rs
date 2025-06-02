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
use crate::{allocator, timer};
use graphic::ansi;
use stream::OutputStream;
use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr;
use log::{Level, Metadata, Record};
use spin::RwLock;
use crate::built_info;

pub struct Logger {
    level: Level,
    streams: RwLock<Vec<Arc<dyn OutputStream>>>,
    serial: Option<SerialPort>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level = record.metadata().level();
        let file = record.file().unwrap_or("unknown").split('/').next_back().unwrap_or("unknown");
        let line = record.line().unwrap_or(0);

        let streams = self.streams.read();
        if streams.is_empty() {
            if let Some(serial) = &self.serial {
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
                    serial.write_str(record.args().as_str().unwrap_or("Formatted messages are not supported before heap initialization!"));
                }

                serial.write_str("\n");
            }
        } else {
            let systime = timer().systime_ms();
            let seconds = systime / 1000;
            let fraction = systime % 1000;

            let string = format!("{}[{}.{:0>3}]{}[{}]{}[{}@{:0>3}]{} {}\n", ansi::FOREGROUND_CYAN, seconds, fraction,
                                 ansi_color(level), level_token(level),ansi::FOREGROUND_MAGENTA, file, line, ansi::FOREGROUND_DEFAULT, record.args());

            for stream in streams.iter() {
                stream.write_str(&string);
            }
        }
    }

    fn flush(&self) {}
}

impl Logger {
    pub fn new() -> Self {
        let mut serial = None;
        if serial::check_port(ComPort::Com1) {
            serial = Some(SerialPort::new_write_only(ComPort::Com1))
        } else if serial::check_port(ComPort::Com2) {
            serial = Some(SerialPort::new_write_only(ComPort::Com2))
        } else if serial::check_port(ComPort::Com3) {
            serial = Some(SerialPort::new_write_only(ComPort::Com3))
        } else if serial::check_port(ComPort::Com4) {
            serial = Some(SerialPort::new_write_only(ComPort::Com4))
        }

        Self {
            level: if built_info::PROFILE == "debug" { Level::Debug } else { Level::Info },
            streams: RwLock::new(Vec::new()),
            serial
        }
    }

    pub fn register(&self, stream: Arc<dyn OutputStream>) {
        self.streams.write().push(stream);
    }

    pub fn remove(&self, stream: &dyn OutputStream) {
        self.streams.write().retain(|element| {
            !ptr::addr_eq(ptr::from_ref(element.as_ref()), ptr::from_ref(stream))
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
