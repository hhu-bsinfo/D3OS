#![no_std]

extern crate alloc;

use alloc::format;
use log::{Level, Log, Metadata, Record};
use syscall::{SystemCall, syscall};

#[cfg(feature = "userspace")]
use spin::Once;

#[cfg(feature = "userspace")]
static LOGGER: Once<Logger> = Once::new();

#[cfg(feature = "userspace")]
pub fn init_logger() {
    use log::{set_logger, LevelFilter};

    LOGGER.call_once(Logger::new);
    set_logger(LOGGER.get().unwrap())
        .map(|()| log::set_max_level(LevelFilter::Debug))
        .expect("Failed to initialize logger!");
}

/// Forward log to kernel logger
pub struct Logger {
    /// the verbosity
    level: Level,
}

impl Log for Logger {
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
        let message = format!("[{}@{:0>3}] {}", file, line, record.args());

        syscall(
            SystemCall::Log,
            &[message.as_bytes().as_ptr() as usize, message.len(), level as usize],
        )
        .unwrap_or_else(|_| panic!("Unable to log {}", message));
    }

    fn flush(&self) {}
}

impl Logger {
    pub const fn new() -> Self {
        Self {
            level: Level::Debug,
        }
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}
