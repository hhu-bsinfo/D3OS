/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscall interface in user mode.                                 ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Michael Schoettner, 26.12.2025, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]
#![feature(variant_count)]

pub mod return_vals;
pub mod syscall_args;

use core::mem;
use crate::return_vals::SyscallResult;
use crate::syscall_args::SyscallArgs;


/// Enum with all known system calls
#[repr(u16)] // Cannot use full size of rax, because ax is needed to set up fs/gs in syscall_handler()
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum SystemCall {
    TerminalReadInput = 0,
    TerminalWriteInput,
    TerminalCheckInputState,
    TerminalWriteOutput,
    TerminalReadOutput,
    MapMemory,
    MapFrameBuffer,
    ProcessExecuteBinary,
    ProcessId,
    ProcessExit,
    ProcessCount,
    ProcessStatus,
    ThreadCreate,
    ThreadId,
    ThreadSwitch,
    ThreadSleep,
    ThreadJoin,
    ThreadExit,
    ThreadKill,
    ThreadCount,
    CoreId,
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
    SockCanSend,
    SockReceive,
    SockCanReceive,
    SockClose,
    GetIpAddresses,
    Mkfifo,
    WriteGraphic,
    GetGraphicResolution,
    MouseRead,
    KeyboardRead,
    MapSystemInfo,
    Log,
    ShmOpen,
    ShmAttach,
    ShmDetach,
    ShmUnlink,
    IpcRegister,
    IpcSend,
    IpcReceiveBlocking,
    IpcDelete,
}

pub const NUM_SYSCALLS: usize = mem::variant_count::<SystemCall>() as usize;

///
/// Description:
///    All syscalls are fired here. Parameters are passed in
///    registers according to the AMD 64 bit ABI.
/// 
/// Parameters:
///     Accepts 0 to 6 arguments using an `usize` slice, as well as `raw ptr` and `ref`.
///     See `SyscallArgs` for more details.
///
/// Return: Result \
///    success >= 0 \
///    error, codes defined in consts.rs
#[cfg(all(target_arch = "x86_64", feature = "userspace"))]
pub fn syscall<T: SyscallArgs>(call: SystemCall, syscall_args: &T) -> SyscallResult {
    use core::arch::asm;
    use return_vals::convert_ret_code_to_syscall_result;

    let ret_code: isize;

    let a0 = syscall_args.arg(0);
    let a1 = syscall_args.arg(1);
    let a2 = syscall_args.arg(2);
    let a3 = syscall_args.arg(3);
    let a4 = syscall_args.arg(4);
    let a5 = syscall_args.arg(5);

    #[cfg(target_arch = "x86_64")]
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
#[cfg(not(all(target_arch = "x86_64", feature = "userspace")))]
pub fn syscall(_call: SystemCall, _args: &[usize]) -> SyscallResult {
    use crate::return_vals::Errno;

    Err(Errno::ENOTSUP)
}