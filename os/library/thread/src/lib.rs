#![no_std]

use syscall::{syscall0, syscall1, SystemCall};

pub fn process_id() -> usize {
    syscall0(SystemCall::ProcessId)
}

pub fn thread_id() -> usize {
    syscall0(SystemCall::ThreadId)
}

#[allow(dead_code)]
pub fn switch() {
    syscall0(SystemCall::ThreadSwitch);
}

#[allow(dead_code)]
pub fn sleep(ms: usize) {
    syscall1(SystemCall::ThreadSleep, ms);
}

pub fn exit() -> ! {
    syscall0(SystemCall::ThreadExit);
    panic!("System call 'ThreadExit' has returned!")
}
