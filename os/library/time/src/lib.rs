/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for everything related to time.                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 31.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

use chrono::{DateTime, TimeDelta, Utc};
use syscall::{syscall, SystemCall};

pub fn systime() -> TimeDelta {
    let res = syscall(SystemCall::GetSystemTime, &[]);
    match res {
        Ok(systime) => TimeDelta::try_milliseconds(systime as i64).expect("Failed to create TimeDelta struct from systime"),
        Err(_) => panic!("Syscall: GetSystemTime failed."),
    }    
}

pub fn date() -> DateTime<Utc> {
    let res = syscall(SystemCall::GetDate, &[]);
    match res {
        Ok(date_ms) => DateTime::from_timestamp_millis(date_ms as i64).expect("Failed to parse date from milliseconds returned by system call"),
        Err(_) => panic!("Syscall: GetDate failed."),
    }    
}

pub fn set_date(date: DateTime<Utc>) -> bool {
    let date_ms = date.timestamp_millis();

    let res = syscall(SystemCall::SetDate, &[date_ms as usize, ]);
    match res {
        Ok(_) => true,
        Err(_) => false,
    }    

}