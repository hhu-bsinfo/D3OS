/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_concurrent                                                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls related to processes and threads.              ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 30.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::vec::Vec;
use alloc::sync::Arc;
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use x86_64::VirtAddr;
use syscall::return_vals::Errno;
use crate::{initrd, process_manager, scheduler};
use crate::process::thread::Thread;


pub fn sys_process_id() -> isize {
    process_manager().read().current_process().id() as isize
}

pub fn sys_process_exit() -> isize {
    scheduler().current_thread().process().exit();
    scheduler().exit();
    0
}

pub fn sys_thread_create(kickoff_addr: u64, entry: fn()) -> isize {
    let thread = Thread::new_user_thread(process_manager().read().current_process(), VirtAddr::new(kickoff_addr), entry);
    let id = thread.id();

    scheduler().ready(thread);
    id as isize
}

pub fn sys_thread_id() -> isize {
    scheduler().current_thread().id() as isize
}

pub fn sys_thread_switch() -> isize {
    scheduler().switch_thread_no_interrupt();
    0
}

pub fn sys_thread_sleep(ms: usize) -> isize {
    scheduler().sleep(ms);
    0
}

pub fn sys_thread_join(id: usize) -> isize {
    scheduler().join(id);
    0
}

pub fn sys_thread_exit() -> isize {
    scheduler().exit();
    0
}

pub fn sys_process_execute_binary(name_buffer: *const u8, name_length: usize, args: *const Vec<&str>) -> isize {
    let app_name = from_utf8(unsafe { slice_from_raw_parts(name_buffer, name_length).as_ref().unwrap() }).unwrap();
    match initrd().entries().find(|entry| entry.filename().as_str().unwrap() == app_name) {
        Some(app) => {
            let thread = Thread::load_application(app.data(), app_name, unsafe { args.as_ref().unwrap() });
            scheduler().ready(Arc::clone(&thread));
            thread.id() as isize
        }
        None => Errno::ENOENT.into(),
    }
}
