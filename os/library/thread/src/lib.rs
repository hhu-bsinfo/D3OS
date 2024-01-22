#![no_std]

use library_syscall::{syscall0, syscall1, SystemCall};

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
