#![no_std]

use core::arch::asm;
use crate::SystemCall::ThreadExit;

#[repr(u8)]
#[allow(dead_code)]
pub enum SystemCall {
    Read = 0,
    Write,
    ProcessId,
    ThreadId,
    ThreadSwitch,
    ThreadSleep,
    ThreadExit,
}

pub const NUM_SYSCALLS: usize = ThreadExit as usize + 1;

#[inline(always)]
pub fn syscall0(arg0: u64) -> u64 {
    let ret: u64;

    unsafe {
        asm!(
        "syscall",
        inlateout("rax") arg0 => ret,
        out("rcx") _,
        out("r11") _,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

#[inline(always)]
pub fn syscall1(arg0: u64, arg1: u64) -> u64 {
    let ret: u64;

    unsafe {
        asm!(
        "syscall",
        inlateout("rax") arg0 => ret,
        in("rdi") arg1,
        out("rcx") _,
        out("r11") _,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

#[inline(always)]
#[allow(dead_code)]
pub fn syscall2(arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let ret: u64;

    unsafe {
        asm!(
        "syscall",
        inlateout("rax") arg0 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        out("rcx") _,
        out("r11") _,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

#[inline(always)]
#[allow(dead_code)]
pub fn syscall3(arg0: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let ret: u64;

    unsafe {
        asm!(
        "syscall",
        inlateout("rax") arg0 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        out("rcx") _,
        out("r11") _,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}
