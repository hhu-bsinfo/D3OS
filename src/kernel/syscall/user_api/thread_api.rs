use crate::kernel;
use crate::kernel::syscall::user_api::{syscall0, syscall1, SystemCall};

#[no_mangle]
pub extern "C" fn sys_thread_switch() {
    kernel::scheduler().switch_thread();
}

#[no_mangle]
pub extern "C" fn sys_thread_sleep(ms: usize) {
    kernel::scheduler().sleep(ms);
}

#[no_mangle]
pub extern "C" fn sys_thread_exit() {
    kernel::scheduler().exit();
}

#[allow(dead_code)]
pub fn usr_thread_switch() {
    syscall0(SystemCall::ThreadSwitch as u64);
}

#[allow(dead_code)]
pub fn usr_thread_sleep(ms: usize) {
    syscall1(SystemCall::ThreadSleep as u64, ms as u64);
}

pub fn usr_thread_exit() {
    syscall0(SystemCall::ThreadExit as u64);
}