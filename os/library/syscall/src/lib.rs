#![no_std]

use core::arch::asm;
use crate::SystemCall::SetDate;

#[repr(usize)]
#[allow(dead_code)]
pub enum SystemCall {
    Read = 0,
    Write,
    MapUserHeap,
    ProcessId,
    ThreadId,
    ThreadSwitch,
    ThreadSleep,
    ThreadJoin,
    ThreadExit,
    ApplicationStart,
    GetSystemTime,
    GetDate,
    SetDate
}

pub const NUM_SYSCALLS: usize = SetDate as usize + 1;

#[inline(always)]
pub fn syscall0(call: SystemCall) -> usize {
    let ret: usize;

    unsafe {
        asm!(
        "syscall",
        inlateout("rax") call as usize => ret,
        out("rcx") _,
        out("r11") _,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

#[inline(always)]
pub fn syscall1(call: SystemCall, arg1: usize) -> usize {
    let ret: usize;

    unsafe {
        asm!(
        "syscall",
        inlateout("rax") call as usize => ret,
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
pub fn syscall2(call: SystemCall, arg1: usize, arg2: usize) -> usize {
    let ret: usize;

    unsafe {
        asm!(
        "syscall",
        inlateout("rax") call as usize => ret,
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
pub fn syscall3(call: SystemCall, arg1: usize, arg2: usize, arg3: usize) -> usize {
    let ret: usize;

    unsafe {
        asm!(
        "syscall",
        inlateout("rax") call as usize => ret,
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
