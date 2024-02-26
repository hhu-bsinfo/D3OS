#![no_std]

use chrono::Duration;
use syscall::{syscall0, SystemCall};

pub fn systime() -> Duration {
    let systime = syscall0(SystemCall::SystemTime);
    Duration::milliseconds(systime as i64)
}