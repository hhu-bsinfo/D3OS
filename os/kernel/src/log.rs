/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: log                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Logger implementation. Support one or several output streams.   ║
   ║         Messages are dumped on each output stream.                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Niklas Sombert, HHU                            ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use crate::device::serial;
use crate::device::serial::ComPort;
use crate::device::serial::SerialPort;
use crate::{allocator, timer};
use graphic::ansi;
use log::debug;
use stream::OutputStream;
use core::fmt::Write;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr;
use log::{Level, Metadata, Record};
use thingbuf::recycling::WithCapacity;
use thingbuf::ThingBuf;
use spin::{Mutex, Once};
use crate::built_info;

pub struct Logger {
    /// the verbosity
    level: Level,
    /// The queue messages are placed into. This is lock-free and needs no
    /// additional heap allocations after its creation.
    queue: Once<ThingBuf<String, WithCapacity>>,
    /// The streams to output to.
    streams: Mutex<Vec<Arc<dyn OutputStream>>>,
    /// If there are no streams and no queue (in the very early boot process),
    /// text is instead written directly to the serial port.
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

        if let Some(queue) = self.queue.get() {
            // the system is up and running
            // add our new message to the queue
            if let Ok(mut slot) = queue.push_ref() {
                let systime = timer().systime_ms();
                let seconds = systime / 1000;
                let fraction = systime % 1000;
                // this doesn't allocate outside of the string
                // TODO: somehow make sure that the string doesn't grow
                write!(
                    *slot,
                    "{}[{}.{:0>3}]{}[{}]{}[{}@{:0>3}]{} {}\n",
                    ansi::FOREGROUND_CYAN, seconds, fraction, ansi_color(level),
                    level_token(level),ansi::FOREGROUND_MAGENTA, file, line,
                    ansi::FOREGROUND_DEFAULT, record.args()
                );
            }
            // if it's full, silently drop the message
            // there will be a warning on the next write

            // check to see if we may write
            if let Some(streams) = self.streams.try_lock() {
                // take the messages out of the queue and print them
                let was_full = queue.remaining() == 0;
                while let Some(message) = queue.pop_ref() {
                    write_message_to_all_streams(&message, &streams);
                }
                if was_full {
                    write_message_to_all_streams("*** log buffer full; some messages have been dropped ***\n", &streams);
                }
            }
        } else {
            // we're very early in the boot process, we might not even have a heap yet
            // so just try to write to the serial port
            if let Some(serial) = &self.serial {
                // this could become garbled if we had multiple threads,
                // but we do not have them at this point
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
        }
    }

    fn flush(&self) {}
}

fn write_message_to_all_streams(string: &str, streams: &spin::MutexGuard<'_, Vec<Arc<dyn OutputStream>>>) {
    for stream in streams.iter() {
        stream.write_str(string);
    }
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
            queue: Once::new(),
            streams: Mutex::new(Vec::new()),
            serial
        }
    }

    pub fn register(&self, stream: Arc<dyn OutputStream>) {
        // make sure we have a queue
        self.queue.call_once(|| {
            debug!("allocating log buffer");
            // fill the buffer with a fixed size of fixed-size strings
            const MESSAGE_LENGTH: usize = 4096;
            let recycle = WithCapacity::new().with_min_capacity(MESSAGE_LENGTH);
            const BUFFER_SIZE: usize = 32;
            let buf = ThingBuf::with_recycle(BUFFER_SIZE, recycle);
            // pre-allocate the strings
            while let Ok(_) = buf.push_ref() {}
            while let Some(_) = buf.pop_ref() {}
            buf
        });
        self.streams.lock().push(stream);
    }

    pub fn remove(&self, stream: &dyn OutputStream) {
        self.streams.lock().retain(|element| {
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
