/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_concurrent                                                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls related to processes and threads.              ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 04.01.2026, HHU                                 ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use crate::process::thread::{ProcessLoadError, Thread};
use crate::{process_manager, scheduler};
use alloc::format;
use alloc::slice;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use syscall::return_vals::{self, Errno};
use x86_64::VirtAddr;

pub extern "sysv64" fn sys_process_id() -> isize {
    process_manager().read().current_process().id() as isize
}

pub extern "sysv64" fn sys_process_exit() -> ! {
    scheduler().current_thread().process().exit();
    scheduler().exit();
}

pub extern "sysv64" fn sys_process_count() -> isize {
    process_manager().read().active_process_ids().len() as isize
}

pub extern "sysv64" fn sys_process_status(buffer: *mut u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 {
        return Errno::EINVAL as isize;
    }
    let buf: &mut [u8];
    unsafe {
        buf = slice::from_raw_parts_mut(buffer, buffer_length);
    }
    return_vals::convert_syscall_result_to_ret_code(scheduler().get_status(buf))
}

pub extern "sysv64" fn sys_thread_create(kickoff_addr: u64, entry: extern "sysv64" fn()) -> isize {
    let thread = Thread::new_user_thread(process_manager().read().current_process(), VirtAddr::new(kickoff_addr), entry);
    let id = thread.id();

    scheduler().ready(thread);
    id as isize
}

pub extern "sysv64" fn sys_thread_id() -> isize {
    scheduler().current_thread().id() as isize
}

pub extern "sysv64" fn sys_thread_switch() -> isize {
    scheduler().switch_thread_no_interrupt();
    0
}

pub extern "sysv64" fn sys_thread_sleep(ms: usize) -> isize {
    scheduler().sleep(ms);
    0
}

pub extern "sysv64" fn sys_thread_join(id: usize) -> isize {
   return_vals::convert_syscall_result_to_ret_code(scheduler().join(id))
}

pub extern "sysv64" fn sys_thread_kill(id: usize) -> isize {
    scheduler().kill(id);
    0
}

pub extern "sysv64" fn sys_thread_exit() -> ! {
    scheduler().exit();
}

pub extern "sysv64" fn sys_thread_count() -> isize {
    scheduler().active_thread_ids().len() as isize
}

pub unsafe extern "sysv64" fn sys_process_execute_binary(name_buffer: *const u8, name_length: usize, args: *const Vec<&str>) -> isize {
    let app_name = from_utf8(unsafe { slice_from_raw_parts(name_buffer, name_length).as_ref().unwrap() }).unwrap();
    let path = format!("bin/{}", app_name);

    match Thread::load_application(&path, app_name, unsafe { args.as_ref().unwrap() }) {
        Ok(thread) => {
            scheduler().ready(Arc::clone(&thread));
            thread.id() as isize
        }
        Err(ProcessLoadError::NotFound) => Errno::ENOENT.into(),
        Err(ProcessLoadError::ElfInvalid) => Errno::EBADF.into(),
    }
}
