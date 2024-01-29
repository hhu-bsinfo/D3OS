use crate::{scheduler, terminal};
use crate::process::process::current_process;

pub mod syscall_dispatcher;

#[no_mangle]
pub extern "C" fn sys_write(buffer: *const u8, length: usize) {
    let terminal = terminal();
    for i in 0..length {
        unsafe { terminal.write_byte(buffer.offset(i as isize).read()) };
    }
}

#[no_mangle]
pub extern "C" fn sys_process_id() -> usize {
    current_process().id()
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
pub extern "C" fn sys_thread_exit() {
    scheduler().exit();
}