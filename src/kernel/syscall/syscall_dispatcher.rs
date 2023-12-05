use core::arch::asm;
use crate::kernel::syscall::user_api::NUM_SYSCALLS;
use crate::kernel::syscall::user_api::thread_api::{sys_thread_exit, sys_thread_sleep, sys_thread_switch};

extern "C" {
    fn init_syscalls();
}
pub fn init() {
    unsafe { init_syscalls(); }
}

#[no_mangle]
pub static SYSCALL_TABLE: SyscallTable = SyscallTable::new();

#[repr(align(64))]
#[repr(C)]
pub struct SyscallTable {
    handle: [*const usize; NUM_SYSCALLS],
}

impl SyscallTable {
    pub const fn new() -> Self {
        SyscallTable {
            handle: [
                sys_thread_switch as *const _,
                sys_thread_sleep as *const _,
                sys_thread_exit as *const _
            ],
        }
    }
}

unsafe impl Send for SyscallTable {}
unsafe impl Sync for SyscallTable {}

#[no_mangle]
#[naked]
pub unsafe extern "C" fn syscall_disp() {
    asm!(
    "call [{syscall_functable} + 8 * rax]",
    "ret",
    syscall_functable = sym SYSCALL_TABLE,
    options(noreturn));
}
#[no_mangle]
pub unsafe extern "C" fn syscall_abort() {
    let syscall_number: u64;

    asm!(
    "mov {}, rax", out(reg) syscall_number
    );

    panic!("System call with id [{}] does not exist!", syscall_number);
}