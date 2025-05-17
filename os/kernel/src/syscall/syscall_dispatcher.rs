/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: syscall_dispatcher                                              ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Low-level dispatcher for system calls.                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 27.12.2024, HHU                                 ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::arch::{asm, naked_asm};
use core::mem::size_of;
use core::ops::Deref;
use core::ptr;
use syscall::NUM_SYSCALLS;
use x86_64::registers::control::{Efer, EferFlags};
use x86_64::registers::model_specific::{KernelGsBase, LStar, Star};
use x86_64::structures::gdt::SegmentSelector;
use x86_64::{PrivilegeLevel, VirtAddr};
use crate::syscall::sys_vmem::sys_map_user_heap;
use crate::syscall::sys_time::{sys_get_date, sys_get_system_time, sys_set_date, };
use crate::syscall::sys_concurrent::{sys_process_execute_binary, sys_process_exit, sys_process_id, sys_thread_create, sys_thread_exit,
    sys_thread_id, sys_thread_join, sys_thread_sleep, sys_thread_switch};
use crate::syscall::sys_naming::*;
use crate::syscall::sys_input::sys_read_mouse;

use crate::{core_local_storage, tss};

use super::sys_concurrent::{sys_process_count, sys_thread_count, sys_thread_kill};
use super::sys_graphic::{sys_get_graphic_resolution, sys_map_fb_info, sys_write_graphic};
use super::sys_input::sys_read_keyboard;
use super::sys_logger::sys_log;
use super::sys_system_info::sys_map_build_info;
use super::sys_terminal::{sys_log_debug, sys_terminal_check_input_state, sys_terminal_read_input, sys_terminal_read_output, sys_terminal_terminate_operator, sys_terminal_write_input, sys_terminal_write_output};

pub const CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX: u64 = 0x00;
pub const CORE_LOCAL_STORAGE_USER_RSP_INDEX: u64 = 0x08;

#[repr(C, packed)]
pub struct CoreLocalStorage {
    tss_rsp0_ptr: VirtAddr,
    user_rsp: VirtAddr,
}

impl CoreLocalStorage {
    pub const fn new() -> Self {
        Self {
            tss_rsp0_ptr: VirtAddr::zero(),
            user_rsp: VirtAddr::zero(),
        }
    }
}

pub fn init() {
    // Enable system call extensions
    unsafe { Efer::update(|flags| flags.set(EferFlags::SYSTEM_CALL_EXTENSIONS, true)) }

    // Set code and stack segments for syscall
    let cs_syscall = SegmentSelector::new(1, PrivilegeLevel::Ring0);
    let ss_syscall = SegmentSelector::new(2, PrivilegeLevel::Ring0);
    let cs_sysret = SegmentSelector::new(4, PrivilegeLevel::Ring3);
    let ss_sysret = SegmentSelector::new(3, PrivilegeLevel::Ring3);

    if let Err(err) = Star::write(cs_sysret, ss_sysret, cs_syscall, ss_syscall) {
        panic!(
            "System Call: Failed to initialize STAR register (Error: {})",
            err
        )
    }

    // Set rip for syscall
    LStar::write(VirtAddr::new(syscall_handler as u64));

    // Initialize core local storage (accessible via 'swapgs')
    let mut core_local_storage = core_local_storage().lock();
    core_local_storage.tss_rsp0_ptr =
        VirtAddr::new(ptr::from_ref(tss().lock().deref()) as u64 + size_of::<u32>() as u64);
    KernelGsBase::write(VirtAddr::new(
        ptr::from_ref(core_local_storage.deref()) as u64
    ));
}

#[unsafe(no_mangle)]
pub static SYSCALL_TABLE: SyscallTable = SyscallTable::new();

#[repr(C, align(64))]
pub struct SyscallTable {
    handle: [*const usize; NUM_SYSCALLS],
}

impl SyscallTable {
    pub const fn new() -> Self {
        SyscallTable {
            handle: [
                sys_terminal_read_input as *const _,
                sys_terminal_write_input as *const _,
                sys_terminal_check_input_state as *const _,
                sys_terminal_write_output as *const _,
                sys_terminal_read_output as *const _,
                sys_terminal_terminate_operator as *const _,
                sys_map_user_heap as *const _,
                sys_process_execute_binary as *const _,
                sys_process_id as *const _,
                sys_process_exit as *const _,
                sys_process_count as *const _,
                sys_thread_create as *const _,
                sys_thread_id as *const _,
                sys_thread_switch as *const _,
                sys_thread_sleep as *const _,
                sys_thread_join as *const _,
                sys_thread_exit as *const _,
                sys_thread_kill as *const _,
                sys_thread_count as *const _,
                sys_get_system_time as *const _,
                sys_get_date as *const _,
                sys_set_date as *const _,
                sys_open as *const _,
                sys_read as *const _,
                sys_write as *const _,
                sys_seek as *const _,
                sys_close as *const _,
                sys_mkdir as *const _,
                sys_touch as *const _,
                sys_readdir as *const _,
                sys_cwd as *const _,
                sys_cd as *const _,
                sys_log_debug as *const _,
                sys_write_graphic as * const _,
                sys_get_graphic_resolution as * const _,
                sys_read_mouse as * const _,
                sys_read_keyboard as * const _,
                sys_map_fb_info as * const _,
                sys_map_build_info as * const _,
                sys_log as * const _,
            ],
        }
    }
}

unsafe impl Send for SyscallTable {}
unsafe impl Sync for SyscallTable {}

#[naked]
#[unsafe(no_mangle)]
#[allow(unsafe_op_in_unsafe_fn)]
///
/// Description: \
///    This functions does not take any parameters per its declaration,
///    but in reality, it takes at least the system call ID in rax
///    and may take additional parameters for the system call in `rdi`, `rsi` ... \
///    See AMD64 ABI. 
///
/// Return: \
///    Two values in `rax`, `rdx` to reconstruct `Result`in user mode
unsafe extern "C" fn syscall_handler() {
    naked_asm!(
    // We are now in ring 0, but still on the user stack
    // Disable interrupts until we have switched to kernel stack
    "cli",

    // Switch to kernel stack
    "swapgs", // Setup core local storage access via gs base
    "mov gs:[{CORE_LOCAL_STORAGE_USER_RSP_INDEX}], rsp", // Temporarily store user rip in core local storage
    "mov rsp, gs:[{CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX}]", // Load pointer to rsp0 entry of tss from core local storage
    "mov rsp, [rsp]", // Dereference rsp0 pointer to switch to kernel stack
    "push gs:[{CORE_LOCAL_STORAGE_USER_RSP_INDEX}]", // Store user rip on kernel stack (core local storage might be overwritten, when a thread switch occurs during system call execution)
    "swapgs", // Restore gs base

    // Store registers (except rax, which is used for system call ID and return value)
    "push rbx",
    "push rcx", // Contains rip for returning to ring 3
    "push rdx", 
    "push rdi",
    "push rsi",
    "push r8",
    "push r9",
    "push r10",
    "push r11", // Contains eflags for returning to ring 3
    "push r12",
    "push r13",
    "push r14",
    "push r15",

    // copy 4th argument to rcx to adhere x86_64 ABI
    "mov rcx, r10",

    // Enable interrupts (we are now on the kernel stack and can handle them properly)
    "sti",

    // Check if system call ID is in bounds
    "cmp rax, {NUM_SYSCALLS}",
    "jge syscall_abort", // Panics and does not return

    // Call system call handler, corresponding to ID (in rax)
    "call syscall_disp",

    // Restore registers
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11", // Contains eflags for returning to ring 3
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rsi",
    "pop rdi",
    "pop rdx",
    "pop rcx", // Contains rip for returning to ring 3
    "pop rbx",

    // Switch back to user stack
    "cli", // Disable interrupts, since we are still in Ring 0 and no interrupt handler should be called with the user stack
    "pop rsp", // Restore rsp from kernel stack,

    // Return to Ring 3
    // Interrupts will be enabled automatically, because eflags is restored from r11
    "sysretq",
    NUM_SYSCALLS = const NUM_SYSCALLS,
    CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX = const CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX,
    CORE_LOCAL_STORAGE_USER_RSP_INDEX = const CORE_LOCAL_STORAGE_USER_RSP_INDEX
    );
}

#[naked]
#[unsafe(no_mangle)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "C" fn syscall_disp() {
    naked_asm!(
    "call [{SYSCALL_TABLE} + 8 * rax]",
    "ret",
    SYSCALL_TABLE = sym SYSCALL_TABLE
    );
}

#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_abort() {
    let syscall_number: u64;

    unsafe {
        asm!(
        "mov {}, rax", out(reg) syscall_number
        );
    }

    panic!("System call with id [{}] does not exist!", syscall_number);
}
