/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls (starting with sys_).                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 29.8.2024, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use crate::memory::r#virtual::{VirtualMemoryArea, VmaType};
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::naming::name_service;
use crate::process::thread::Thread;
use crate::{efi_system_table, initrd, process_manager, scheduler, terminal, timer};
use alloc::format;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec;
use chrono::{DateTime, Datelike, TimeDelta, Timelike};
use core::ptr;
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use uefi::table::runtime::{Time, TimeParams};
use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr; // for naming service, for the time being

pub mod syscall_dispatcher;


///
/// Description: Used by `convert_syscall_result_to_codes`
///
/// Parameters:  `t`     data to be returned
///
/// Return:      `(0, t)`
#[inline]
fn zero_ok<T: Into<usize>>(t: T) -> (usize, usize) {
    (0, t.into())
}

#[inline]
fn one_err<E: Into<usize>>(e: E) -> (usize, usize) {
    (1, e.into())
}


///
/// Description:
///    Converts a Result to a (usize,usize) tuple
///
/// Parameters: \
///   `result` Result to be converted \
///   `ok_f`  function to produce the content for `Ok` \
///   `err_f` function to produce the content for `Err`
#[inline]
fn convert_syscall_result_to_codes<T, E, F, G>(
    result: Result<T, E>,
    ok_f: F,
    err_f: G,
) -> (usize, usize)
where
    F: Fn(T) -> (usize, usize),
    G: Fn(E) -> (usize, usize),
{
    match result {
        Ok(t) => ok_f(t),
        Err(e) => err_f(e),
    }
}

pub fn sys_read() -> usize {
    let terminal = terminal();
    match terminal.read_byte() {
        -1 => panic!("Input stream closed!"),
        c => c as usize
    }
}

pub fn sys_write(buffer: *const u8, length: usize) {
    let string = from_utf8(unsafe { slice_from_raw_parts(buffer, length).as_ref().unwrap() }).unwrap();
    let terminal = terminal();
    terminal.write_str(string);
}

pub fn sys_map_user_heap(size: usize) -> usize {
    let process = process_manager().read().current_process();
    let code_areas = process.find_vmas(VmaType::Code);
    let code_area = code_areas.get(0).expect("Process does not have code area!");
    let heap_start = code_area.end().align_up(PAGE_SIZE as u64);
    let heap_area = VirtualMemoryArea::from_address(heap_start, size, VmaType::Heap);

    process.address_space().map(
        heap_area.range(),
        MemorySpace::User,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
    );
    process.add_vma(heap_area);

    heap_start.as_u64() as usize
}

pub fn sys_process_id() -> usize {
    process_manager().read().current_process().id()
}

pub fn sys_process_exit() {
    scheduler().current_thread().process().exit();
    scheduler().exit();
}

#[allow(improper_ctypes_definitions)] // 'entry' takes no arguments and has no return value, so we just assume that the "C" and "Rust" ABIs act the same way in this case
pub fn sys_thread_create(kickoff_addr: u64, entry: fn()) -> usize {
    let thread = Thread::new_user_thread(process_manager().read().current_process(), VirtAddr::new(kickoff_addr), entry);
    let id = thread.id();

    scheduler().ready(thread);
    id
}

pub fn sys_thread_id() -> usize {
    scheduler().current_thread().id()
}

pub fn sys_thread_switch() {
    scheduler().switch_thread_no_interrupt();
}

pub fn sys_thread_sleep(ms: usize) {
    scheduler().sleep(ms);
}

pub fn sys_thread_join(id: usize) {
    scheduler().join(id);
}

pub fn sys_thread_exit() {
    scheduler().exit();
}

pub fn sys_process_execute_binary(name_buffer: *const u8, name_length: usize) -> usize {
    let app_name = from_utf8(unsafe {
        slice_from_raw_parts(name_buffer, name_length)
            .as_ref()
            .unwrap()
    })
    .unwrap();
    match initrd()
        .entries()
        .find(|entry| entry.filename().as_str().unwrap() == app_name)
    {
        Some(app) => {
            let thread = Thread::load_application(app.data());
            scheduler().ready(Rc::clone(&thread));
            thread.id()
        }
        None => 0,
    }
}

pub fn sys_get_system_time() -> usize {
    timer().systime_ms()
}

pub fn sys_get_date() -> usize {
    if let Some(efi_system_table) = efi_system_table() {
        let system_table = efi_system_table.read();
        let runtime_services = unsafe { system_table.runtime_services() };

        return match runtime_services.get_time() {
            Ok(time) => {
                if time.is_valid().is_ok() {
                    let timezone = match time.time_zone() {
                        Some(timezone) => {
                            let delta = TimeDelta::try_minutes(timezone as i64)
                                .expect("Failed to create TimeDelta struct from timezone");
                            if timezone >= 0 {
                                format!(
                                    "+{:0>2}:{:0>2}",
                                    delta.num_hours(),
                                    delta.num_minutes() % 60
                                )
                            } else {
                                format!(
                                    "-{:0>2}:{:0>2}",
                                    delta.num_hours(),
                                    delta.num_minutes() % 60
                                )
                            }
                        }
                        None => "Z".to_string(),
                    };

                    DateTime::parse_from_rfc3339(
                        format!("{}-{:0>2}-{:0>2}T{:0>2}:{:0>2}:{:0>2}.{:0>9}{}", time.year(), time.month(), time.day(), time.hour(), time.minute(), time.second(), time.nanosecond(), timezone).as_str())
                        .expect("Failed to parse date from EFI runtime services")
                        .timestamp_millis() as usize
                } else {
                    0
                }
            }
            Err(_) => 0
        }
    }

    0
}

pub fn sys_set_date(date_ms: usize) -> usize {
    if let Some(efi_system_table) = efi_system_table() {
        let system_table = efi_system_table.write();
        let runtime_services_read = unsafe { system_table.runtime_services() };
        let runtime_services = unsafe {
            ptr::from_ref(runtime_services_read)
                .cast_mut()
                .as_mut()
                .unwrap()
        };

        let date = DateTime::from_timestamp_millis(date_ms as i64)
            .expect("Failed to parse date from milliseconds");
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
        })
        .expect("Failed to create EFI date");

        return match unsafe { runtime_services.set_time(&uefi_date) } {
            Ok(_) => true as usize,
            Err(_) => false as usize,
        };
    }

    false as usize
}

pub fn sys_mkentry(
    path_buff: *const u8,
    path_buff_len: usize,
    name_buff: *const u8,
    name_buff_len: usize,
    data: usize,
) -> (usize, usize) {
    let path = from_utf8(unsafe {
        slice_from_raw_parts(path_buff, path_buff_len)
            .as_ref()
            .unwrap()
    })
    .unwrap();
    let name = from_utf8(unsafe {
        slice_from_raw_parts(name_buff, name_buff_len)
            .as_ref()
            .unwrap()
    })
    .unwrap();
    let r = name_service::mkentry_new(path, name, vec![1]);
    convert_syscall_result_to_codes(r, zero_ok, one_err)

    //info!("sys_mkentry({}, {}, {}, {}, {})", arg1, arg2, arg3, arg4, arg5);
//    return (0xAA, 0xBB);
}
