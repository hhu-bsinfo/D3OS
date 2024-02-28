use alloc::format;
use alloc::rc::Rc;
use alloc::string::ToString;
use core::ptr;
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use chrono::{Datelike, DateTime, TimeDelta, Timelike};
use uefi::table::runtime::{Time, TimeParams};
use x86_64::structures::paging::PageTableFlags;
use crate::{efi_system_table, initrd, process_manager, scheduler, terminal, timer};
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::memory::r#virtual::{VirtualMemoryArea, VmaType};
use crate::process::thread::Thread;

pub mod syscall_dispatcher;

#[no_mangle]
pub extern "C" fn sys_read() -> usize {
    let terminal = terminal();
    match terminal.read_byte() {
        -1 => panic!("Input stream closed!"),
        c => c as usize
    }
}

#[no_mangle]
pub extern "C" fn sys_write(buffer: *const u8, length: usize) {
    let string = from_utf8(unsafe { slice_from_raw_parts(buffer, length).as_ref().unwrap() }).unwrap();
    let terminal = terminal();
    terminal.write_str(string);
}

#[no_mangle]
pub extern "C" fn sys_map_user_heap(size: usize) -> usize {
    let process = process_manager().read().current_process();
    let code_area = process.find_vma(VmaType::Code).expect("Process does not have code area!");
    let heap_start = code_area.end().align_up(PAGE_SIZE as u64);
    let heap_area = VirtualMemoryArea::from_address(heap_start, size, VmaType::Heap);

    process.address_space().map(heap_area.range(), MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);
    process.add_vma(heap_area);

    return heap_start.as_u64() as usize;
}

#[no_mangle]
pub extern "C" fn sys_process_id() -> usize {
    process_manager().read().current_process().id()
}

#[no_mangle]
pub extern "C" fn sys_thread_id() -> usize {
    scheduler().current_thread().id()
}

#[no_mangle]
pub extern "C" fn sys_thread_switch() {
    scheduler().switch_thread();
}

#[no_mangle]
pub extern "C" fn sys_thread_sleep(ms: usize) {
    scheduler().sleep(ms);
}

#[no_mangle]
pub extern "C" fn sys_thread_join(id: usize) {
    scheduler().join(id);
}

#[no_mangle]
pub extern "C" fn sys_thread_exit() {
    scheduler().exit();
}

#[no_mangle]
pub extern "C" fn sys_application_start(name_buffer: *const u8, name_length: usize) -> usize {
    let app_name = from_utf8(unsafe { slice_from_raw_parts(name_buffer, name_length).as_ref().unwrap() }).unwrap();
    match initrd().entries().find(|entry| entry.filename().as_str() == app_name) {
        Some(app) => {
            let thread = Thread::new_user_thread(app.data());
            scheduler().ready(Rc::clone(&thread));
            thread.id()
        }
        None => 0
    }
}

#[no_mangle]
pub extern "C" fn sys_get_system_time() -> usize {
    timer().read().systime_ms()
}

#[no_mangle]
pub extern "C" fn sys_get_date() -> usize {
    if let Some(efi_system_table) = efi_system_table() {
        let system_table = efi_system_table.read();
        let runtime_services = unsafe { system_table.runtime_services() };

        match runtime_services.get_time() {
            Ok(time) => {
                if time.is_valid() {
                    let timezone = match time.time_zone() {
                        Some(timezone) => {
                            let delta = TimeDelta::minutes(timezone as i64);
                            if timezone >= 0 {
                                format!("+{:0>2}:{:0>2}", delta.num_hours(), delta.num_minutes() % 60)
                            } else {
                                format!("-{:0>2}:{:0>2}", delta.num_hours(), delta.num_minutes() % 60)
                            }
                        }
                        None => "Z".to_string(),
                    };

                    return DateTime::parse_from_rfc3339(
                        format!("{}-{:0>2}-{:0>2}T{:0>2}:{:0>2}:{:0>2}.{:0>9}{}", time.year(), time.month(), time.day(), time.hour(), time.minute(), time.second(), time.nanosecond(), timezone).as_str())
                        .expect("Failed to parse date from EFI runtime services")
                        .timestamp_millis() as usize
                } else {
                    return 0;
                }
            }
            Err(_) => return 0
        }
    }

    return 0;
}

#[no_mangle]
pub extern "C" fn sys_set_date(date_ms: usize) -> usize {
    if let Some(efi_system_table) = efi_system_table() {
        let system_table = efi_system_table.write();
        let runtime_services_read = unsafe { system_table.runtime_services() };
        let runtime_services = unsafe { ptr::from_ref(runtime_services_read).cast_mut().as_mut().unwrap() };

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
            daylight: Default::default() }).expect("Failed to create EFI date");

        return match unsafe { runtime_services.set_time(&uefi_date) } {
            Ok(_) => true as usize,
            Err(_) => false as usize
        }
    }

    return false as usize;
}