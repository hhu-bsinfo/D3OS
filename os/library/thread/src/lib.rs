#![no_std]

use syscall::{syscall0, syscall1, SystemCall};

#[allow(dead_code)]
pub fn switch() {
    syscall0(SystemCall::ThreadSwitch as u64);
}

#[allow(dead_code)]
pub fn sleep(ms: usize) {
    syscall1(SystemCall::ThreadSleep as u64, ms as u64);
}

pub fn exit() -> ! {
    syscall0(SystemCall::ThreadExit as u64);
    panic!("System call 'ThreadExit' has returned!")
}
