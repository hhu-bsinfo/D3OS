/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls (starting with sys_).                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 30.8.2024, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::format;
use alloc::string::ToString;
use chrono::{DateTime, Datelike, TimeDelta, Timelike};
use uefi::runtime::{Time, TimeParams};
use crate::{efi_services_available, timer};


pub fn sys_get_system_time() -> isize {
    timer().systime_ms() as isize
}

pub fn sys_get_date() -> isize {
    if !efi_services_available() {
        return 0;
    }
    
    match uefi::runtime::get_time() {
        Ok(time) => {
            if time.is_valid().is_ok() {
                let timezone = match time.time_zone() {
                    Some(timezone) => {
                        let delta = TimeDelta::try_minutes(timezone as i64).expect("Failed to create TimeDelta struct from timezone");
                        if timezone >= 0 {
                            format!("+{:0>2}:{:0>2}", delta.num_hours(), delta.num_minutes() % 60)
                        } else {
                            format!("-{:0>2}:{:0>2}", delta.num_hours(), delta.num_minutes() % 60)
                        }
                    }
                    None => "Z".to_string(),
                };

                DateTime::parse_from_rfc3339(format!("{}-{:0>2}-{:0>2}T{:0>2}:{:0>2}:{:0>2}.{:0>9}{}", time.year(), time.month(), time.day(), time.hour(), time.minute(), time.second(), time.nanosecond(), timezone).as_str())
                    .expect("Failed to parse date from EFI runtime services")
                    .timestamp_millis() as isize
            } else {
                0
            }
        }
        Err(_) => 0
    }
}

pub fn sys_set_date(date_ms: usize) -> isize {
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
