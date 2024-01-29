#![no_std]

use syscall::{syscall0, syscall1, SystemCall};

pub fn process_id() -> usize {
    syscall0(SystemCall::ProcessId as u64) as usize
}

pub fn thread_id() -> usize {
    syscall0(SystemCall::ThreadId as u64) as usize
}

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
