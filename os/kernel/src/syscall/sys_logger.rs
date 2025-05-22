use core::slice::from_raw_parts;

use alloc::string::String;
use log::{Level, error, log};
use logger::LogLevel;
use syscall::return_vals::Errno;

pub fn sys_log(address: *const u8, length: usize, level: usize) -> isize {
    if address.is_null() {
        error!("Unable to read userspace log");
        return Errno::EINVAL as isize;
    }

    let bytes = unsafe { from_raw_parts(address, length) };
    let message = String::from_utf8_lossy(bytes);

    let lvl: Level = match LogLevel::from(level) {
        LogLevel::Error => Level::Error,
        LogLevel::Warn => Level::Warn,
        LogLevel::Info => Level::Info,
        LogLevel::Debug => Level::Debug,
        LogLevel::Trace => Level::Trace,
    };

    log!(lvl, "{}", message);
    0
}
