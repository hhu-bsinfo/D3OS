/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscall interface in user mode.                                 ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Michael Schoettner, 25.8.2025, HHU              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

use crate::return_vals::SyscallResult;

pub mod return_vals;

/// Enum with all known system calls
#[repr(usize)]
#[allow(dead_code)]
pub enum SystemCall {
    TerminalRead = 0,
    TerminalReadNonBlocking,
    TerminalWrite,
    MapMemory,
    MapFrameBuffer,
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
    Open,
    Read,
    Write,
    Seek,
    Close,
    MkDir,
    Touch,
    Readdir,
    Cwd,
    Cd,
    SockOpen,
    SockBind,
    SockAccept,
    SockConnect,
    SockSend,
    SockReceive,
    SockClose,
    GetIpAddresses,
    Mkfifo,
    // no syscall, just marking last number, see NUM_SYSCALLS
    // insert any new system calls before this marker
    LastEntryMarker,
}

pub const NUM_SYSCALLS: usize = SystemCall::LastEntryMarker as usize;

///
/// Description:
///    All syscalls are fired here. Parameters are passed in 
///    registers according to the AMD 64 bit ABI.
///
/// Return: Result \
///    success >= 0 \
///    error, codes defined in consts.rs
#[cfg(target_arch = "x86_64")]
pub fn syscall(call: SystemCall, args: &[usize]) -> SyscallResult {
    use core::arch::asm;
    use return_vals::convert_ret_code_to_syscall_result;

    let ret_code: isize;

    if args.len() > 6 {
        panic!("System calls with more than 6 params are not supported.");
    }

    let a0 = *args.first().unwrap_or(&0usize);
    let a1 = *args.get(1).unwrap_or(&0usize);
    let a2 = *args.get(2).unwrap_or(&0usize);
    let a3 = *args.get(3).unwrap_or(&0usize);
    let a4 = *args.get(4).unwrap_or(&0usize);
    let a5 = *args.get(5).unwrap_or(&0usize);

    unsafe {
        asm!(
            "syscall",
            inlateout("rax") call as i64 => ret_code,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            in("r10") a3,
            in("r8") a4,
            in("r9") a5,
            lateout("rcx") _,
            lateout("r11") _,
            clobber_abi("system"));
    }

    convert_ret_code_to_syscall_result(ret_code)
}

/// Only needed to run tests on non-x86_64 architectures (e.g. Apple Silicon).
#[cfg(not(target_arch = "x86_64"))]
pub fn syscall(_call: SystemCall, _args: &[usize]) -> SyscallResult {
    use crate::return_vals::Errno;

    Err(Errno::ENOTSUP)
}