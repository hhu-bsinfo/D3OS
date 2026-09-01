/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls (starting with sys_).                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 30.8.2024, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use chrono::{DateTime, Datelike, Timelike};
use uefi::runtime::{Time, TimeParams};
use crate::{efi_services_available, now, timer};


pub extern "sysv64" fn sys_get_system_time() -> isize {
    timer().systime_ms() as isize
}

pub extern "sysv64" fn sys_get_date() -> isize {
    if !efi_services_available() {
        return 0;
    }
    
    match now() {
        Some(datetime) => datetime.timestamp_millis() as isize,
        None => 0,
    }
}

pub extern "sysv64" fn sys_set_date(date_ms: usize) -> isize {
    let date = DateTime::from_timestamp_millis(date_ms as i64).expect("Failed to parse date from milliseconds");
    let uefi_date = Time::new(TimeParams {
        year: date.year() as u16,
        month: date.month() as u8,
        day: date.day() as u8,
        hour: date.hour() as u8,
        minute: date.minute() as u8,
        second: date.second() as u8,
        nanosecond: date.nanosecond(),
        time_zone: None,
        daylight: Default::default(),
    }).expect("Failed to create EFI date");

    match unsafe { uefi::runtime::set_time(&uefi_date) } {
        Ok(_) => true as isize,
        Err(_) => false as isize,
    }
}
