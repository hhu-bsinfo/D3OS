use alloc::format;
use alloc::rc::Rc;
use alloc::string::ToString;
use drawer::drawer::DrawerCommand;
use graphic::color::BLACK;
use io::Application;
use libm::Libm;
use stream::InputStream;
use core::f32::consts::PI;
use core::mem::size_of;
use core::ptr;
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use chrono::{Datelike, DateTime, TimeDelta, Timelike};
use uefi::table::runtime::{Time, TimeParams};
use x86_64::structures::paging::PageTableFlags;
use crate::{buffered_lfb, efi_system_table, initrd, process_manager, ps2_devices, scheduler, terminal, timer};
use x86_64::VirtAddr;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::memory::r#virtual::{VirtualMemoryArea, VmaType};
use crate::process::thread::Thread;

pub mod syscall_dispatcher;

#[no_mangle]
pub extern "C" fn sys_read(application_ptr: *const Application) -> usize {
    let enum_val = unsafe { application_ptr.as_ref().unwrap() };
    match enum_val {
        Application::Shell => {
            let terminal = terminal();
            return terminal.read_byte() as usize;
        },
        Application::WindowManager => {
            return ps2_devices().keyboard().read_byte() as usize;
        },
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
    let code_areas = process.find_vmas(VmaType::Code);
    let code_area = code_areas.get(0).expect("Process does not have code area!");
    let heap_start = code_area.end().align_up(PAGE_SIZE as u64);
    let heap_area = VirtualMemoryArea::from_address(heap_start, size, VmaType::Heap);

    process.address_space().map(
        heap_area.range(), 
        MemorySpace::User, 
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE
    );
    process.add_vma(heap_area);

    return heap_start.as_u64() as usize;
}

#[no_mangle]
pub extern "C" fn sys_process_id() -> usize {
    process_manager().read().current_process().id()
}

#[no_mangle]
pub extern "C" fn sys_process_exit() {
    scheduler().current_thread().process().exit();
    scheduler().exit();
}

#[no_mangle]
#[allow(improper_ctypes_definitions)] // 'entry' takes no arguments and has no return value, so we just assume that the "C" and "Rust" ABIs act the same way in this case
pub extern "C" fn sys_thread_create(kickoff_addr: u64, entry: fn()) -> usize {
    let thread = Thread::new_user_thread(process_manager().read().current_process(), VirtAddr::new(kickoff_addr), entry);
    let id = thread.id();

    scheduler().ready(thread);
    return id;
}

#[no_mangle]
pub extern "C" fn sys_thread_id() -> usize {
    scheduler().current_thread().id()
}

#[no_mangle]
pub extern "C" fn sys_thread_switch() {
    scheduler().switch_thread_no_interrupt();
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
pub extern "C" fn sys_process_execute_binary(name_buffer: *const u8, name_length: usize) -> usize {
    let app_name = from_utf8(unsafe { slice_from_raw_parts(name_buffer, name_length).as_ref().unwrap() }).unwrap();
    match initrd().entries().find(|entry| entry.filename().as_str().unwrap() == app_name) {
        Some(app) => {
            let thread = Thread::load_application(app.data());
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
                            let delta = TimeDelta::try_minutes(timezone as i64).expect("Failed to create TimeDelta struct from timezone");
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

#[no_mangle]
pub extern "C" fn sys_write_graphic(command_ptr: *const DrawerCommand) -> usize {
    let enum_val = unsafe { command_ptr.as_ref().unwrap() };
    let mut buff_lfb = buffered_lfb().lock();
    let lfb = buff_lfb.lfb();
    match enum_val {
        DrawerCommand::ClearScreen => {
            lfb.clear();
        },
        DrawerCommand::DrawLine { from, to, color } => {
            lfb.draw_line(from.x, from.y, to.x, to.y, color.clone())
        },
        DrawerCommand::DrawPolygon(vertices, color) => {
            let first_vertex = vertices.first();
            let mut prev = match first_vertex {
                Some(unwrapped) => unwrapped,
                None => return 0usize,
            };
            let last_vertex = vertices.last().unwrap();
            for vertex in &vertices[1..] {
                lfb.draw_line(prev.x, prev.y, vertex.x, vertex.y, color.clone());
                prev = vertex;
            }

            lfb.draw_line(last_vertex.x, last_vertex.y, first_vertex.unwrap().x, first_vertex.unwrap().y, color.clone());
        },
        DrawerCommand::DrawCircle { center, radius, color } => {
            let stepsize = PI / 128.0;
            const TWO_PI: f32 = PI * 2.0;
            let mut x_curr = 0.0;
            while x_curr <= TWO_PI {
                lfb.draw_pixel(
                    Libm::<f32>::round(Libm::<f32>::sin(x_curr) * (radius.clone() as f32) + (center.x as f32)) as u32, 
                    Libm::<f32>::round(Libm::<f32>::cos(x_curr) * (radius.clone() as f32) + (center.y as f32)) as u32, 
                    color.clone()
                );

                x_curr += stepsize;
            }
        },
        DrawerCommand::DrawString { string_to_draw, pos, color } => {
            lfb.draw_string(pos.x, pos.y, color.clone(), BLACK, string_to_draw);
        },
        DrawerCommand::DrawChar { char_to_draw, pos, color } => {
            lfb.draw_char(pos.x, pos.y, color.clone(), BLACK, *char_to_draw);
        }
    };

    buff_lfb.flush();

    return 0usize;
}

/// w = width, h = height;
/// Format in bytes: wwwwhhhh
pub extern "C" fn sys_get_graphic_resolution() -> usize {
    // We need 64bits to transform the information of both width and height.
    if size_of::<usize>() != 8 {
        return 0;
    }
    let buffered_lfb = &mut buffered_lfb().lock();
    let lfb = buffered_lfb.direct_lfb();
    return (((lfb.width() as u64) << 32) | (lfb.height() as u64)) as usize;
}