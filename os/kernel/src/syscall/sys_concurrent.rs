/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_concurrent                                                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls related to processes and threads.              ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 30.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::vec::Vec;
use alloc::rc::Rc;
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use x86_64::VirtAddr; 
use crate::{initrd, process_manager, scheduler};
use crate::process::thread::Thread;


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

pub fn sys_process_execute_binary(name_buffer: *const u8, name_length: usize, args: *const Vec<&str>) -> usize {
    let app_name = from_utf8(unsafe { slice_from_raw_parts(name_buffer, name_length).as_ref().unwrap() }).unwrap();
    match initrd().entries().find(|entry| entry.filename().as_str().unwrap() == app_name) {
        Some(app) => {
            let thread = Thread::load_application(app.data(), app_name, unsafe { args.as_ref().unwrap() });
            scheduler().ready(Rc::clone(&thread));
            thread.id()
        }
        None => 0,
    }
}
