use core::slice::from_raw_parts;

use alloc::string::String;
use log::{Level, error, log};
use syscall::return_vals::Errno;

/// SystemCall implementation for SystemCall::Log.
/// Receives logging data from User-Space and forwards it to the kernel logger.
///
/// Author: Sebastian Keller
pub extern "sysv64" fn sys_log(address: *const u8, length: usize, level: Level) -> isize {
    if address.is_null() {
        error!("Unable to read userspace log");
        return Errno::EINVAL as isize;
    }

    let bytes = unsafe { from_raw_parts(address, length) };
    let message = String::from_utf8_lossy(bytes);

    log!(level, "{}", message);
    0
}
