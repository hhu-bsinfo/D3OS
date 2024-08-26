/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscall interface in user mode.                                 ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 22.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

use core::arch::asm;
use crate::SystemCall::Mkentry;

#[repr(usize)]
#[allow(dead_code)]
pub enum SystemCall {
    Read = 0,
    Write,
    MapUserHeap,
    ProcessExecuteBinary,
    ProcessId,
    ProcessExit,
    ThreadCreate,
    ThreadId,
    ThreadSwitch,
    ThreadSleep,
    ThreadJoin,
    ThreadExit,
    GetSystemTime,
    GetDate,
    SetDate,
    Mkentry,
}

pub const NUM_SYSCALLS: usize = Mkentry as usize + 1;

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

#[inline(always)]
#[allow(dead_code)]
pub fn syscall4(call: SystemCall, arg1: usize, arg2: usize, arg3: usize, arg4: usize) -> usize {
    let ret: usize;

    unsafe {
        asm!(
        "syscall",
        inlateout("rax") call as usize => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        out("rcx") _,
        out("r11") _,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}

#[inline(always)]
#[allow(dead_code)]
pub fn syscall5(call: SystemCall, arg1: usize, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> usize {
    let ret: usize;

    unsafe {
        asm!(
        "syscall",
        inlateout("rax") call as usize => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        in("r8") arg5,
        out("rcx") _,
        out("r11") _,
        options(preserves_flags, nostack)
        );
    }

    return ret;
}


/*
/// Tizzler, kernel
pub unsafe fn raw_syscall(call: Syscall, args: &[u64]) -> (u64, u64) {
    if core::intrinsics::unlikely(args.len() > 6) {
        crate::print_err("too many arguments to raw_syscall");
        crate::internal_abort();
    }
    let a0 = *args.first().unwrap_or(&0u64);
    let a1 = *args.get(1).unwrap_or(&0u64);
    let mut a2 = *args.get(2).unwrap_or(&0u64);
    let a3 = *args.get(3).unwrap_or(&0u64);
    let a4 = *args.get(4).unwrap_or(&0u64);
    let a5 = *args.get(5).unwrap_or(&0u64);

    let mut num = call.num();
    core::arch::asm!("syscall", inout("rax") num, in("rdi") a0, in("rsi") a1, inout("rdx") a2, in("r10") a3, in("r9") a4, in("r8") a5, lateout("rcx") _, lateout("r11") _, clobber_abi("system"));
    (num, a2)
}



// im user mode
pub struct ClockInfo {
    current: TimeSpan,
    precision: FemtoSeconds,
    resolution: FemtoSeconds,
    flags: ClockFlags,
}


#[repr(u64)]
// im user mode
/// Possible error returns for [sys_read_clock_info].
pub enum ReadClockInfoError {
    /// An unknown error occurred.
    #[num_enum(default)]
    #[error("unknown error")]
    Unknown = 0,
    /// One of the arguments was invalid.
    #[error("invalid argument")]
    InvalidArgument = 1,
}

// im user mode
pub fn sys_read_clock_info(
    clock_source: ClockSource,
    flags: ReadClockFlags,
) -> Result<ClockInfo, ReadClockInfoError> {
    let mut clock_info = MaybeUninit::uninit();
    let (code, val) = unsafe {
        raw_syscall(
            Syscall::ReadClockInfo,
            &[
                clock_source.into(),
                &mut clock_info as *mut MaybeUninit<ClockInfo> as usize as u64,
                flags.bits() as u64,
            ],
        )
    };
    convert_codes_to_result(
        code,
        val,
        |c, _| c != 0,
        |_, _| unsafe { clock_info.assume_init() },
        |_, v| v.into(),
    )
}

// im user mode
#[inline]
fn convert_codes_to_result<T, E, D, F, G>(code: u64, val: u64, d: D, f: F, g: G) -> Result<T, E>
where
    F: Fn(u64, u64) -> T,
    G: Fn(u64, u64) -> E,
    D: Fn(u64, u64) -> bool,
{
    if d(code, val) {
        Err(g(code, val))
    } else {
        Ok(f(code, val))
    }
}

// im kernel
Syscall::ReadClockInfo => {
    let result = type_read_clock_info(context.arg0(), context.arg1(), context.arg2());
    let (code, val) = convert_result_to_codes(result, zero_ok, one_err);
    context.set_return_values(code, val);
}

// im kernel
#[inline]
fn convert_result_to_codes<T, E, F, G>(result: Result<T, E>, f: F, g: G) -> (u64, u64)
where
    F: Fn(T) -> (u64, u64),
    G: Fn(E) -> (u64, u64),
{
    match result {
        Ok(t) => f(t),
        Err(e) => g(e),
    }
}

*/