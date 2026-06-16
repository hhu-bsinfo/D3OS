#![no_std]

extern crate alloc;

use alloc::format;
use log::{Level, Log, Metadata, Record};
use syscall::{SystemCall, syscall};

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
        .expect(&format!("Unable to log {}", message));
    }

    fn flush(&self) {}
}

impl Logger {
    pub fn new() -> Self {
        Self {
            level: Level::Debug,
        }
    }
}
