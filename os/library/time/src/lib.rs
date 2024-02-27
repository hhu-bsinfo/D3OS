#![no_std]

use chrono::{DateTime, Duration, Utc};
use syscall::{syscall0, syscall1, SystemCall};

pub fn systime() -> Duration {
    let systime = syscall0(SystemCall::GetSystemTime);
    Duration::milliseconds(systime as i64)
}

pub fn date() -> DateTime<Utc> {
    let date_ms = syscall0(SystemCall::GetDate);
    DateTime::from_timestamp_millis(date_ms as i64).expect("Failed to parse date from milliseconds returned by system call!")
}

pub fn set_date(date: DateTime<Utc>) -> bool {
    let date_ms = date.timestamp_millis();
    let success = syscall1(SystemCall::SetDate, date_ms as usize);

    return success != 0;
}