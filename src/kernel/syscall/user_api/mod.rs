use core::arch::asm;
use crate::kernel::syscall::user_api::SystemCall::ThreadExit;

pub mod thread_api;

#[repr(u8)]
#[allow(dead_code)]
pub enum SystemCall {
    ThreadSwitch = 0,
    ThreadSleep = 1,
    ThreadExit = 2,
}

pub const NUM_SYSCALLS: usize = ThreadExit as usize + 1;

#[inline(always)]
pub fn syscall0(arg0: u64) -> u64 {
    let mut ret: u64;

    unsafe {
        asm!("int 0x86",
        inlateout("rax") arg0 => ret,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

#[inline(always)]
pub fn syscall1(arg0: u64, arg1: u64) -> u64 {
    let mut ret: u64;

    unsafe {
        asm!("int 0x86",
        inlateout("rax") arg0 => ret,
        in("rdi") arg1,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

#[inline(always)]
#[allow(dead_code)]
pub fn syscall2(arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let mut ret: u64;

    unsafe {
        asm!("int 0x86",
        inlateout("rax") arg0 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

#[inline(always)]
#[allow(dead_code)]
pub fn syscall3(arg0: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let mut ret: u64;

    unsafe {
        asm!("int 0x86",
        inlateout("rax") arg0 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

#[inline(always)]
#[allow(dead_code)]
pub fn syscall4(arg0: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> u64 {
    let mut ret: u64;

    unsafe {
        asm!("int 0x86",
        inlateout("rax") arg0 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

#[inline(always)]
#[allow(dead_code)]
pub fn syscall5(arg0: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> u64 {
    let mut ret: u64;

    unsafe {
        asm!("int 0x86",
        inlateout("rax") arg0 => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        in("r8") arg5,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

