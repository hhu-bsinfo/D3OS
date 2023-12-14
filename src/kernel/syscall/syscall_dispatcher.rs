use core::arch::asm;
use x86_64::{PrivilegeLevel, VirtAddr};
use x86_64::registers::control::{Efer, EferFlags};
use x86_64::registers::model_specific::{LStar, Star};
use x86_64::structures::gdt::SegmentSelector;
use crate::kernel::syscall::user_api::NUM_SYSCALLS;
use crate::kernel::syscall::user_api::thread_api::{sys_thread_exit, sys_thread_sleep, sys_thread_switch};

extern "C" {
    fn syscall_handler();
}
pub fn init() {
    // Enable system call extensions
    unsafe { Efer::update(|flags| flags.set(EferFlags::SYSTEM_CALL_EXTENSIONS, true)); }

    // Set code and stack segments for syscall
    let cs_syscall = SegmentSelector::new(2, PrivilegeLevel::Ring0);
    let ss_syscall = SegmentSelector::new(3, PrivilegeLevel::Ring0);
    let cs_sysret = SegmentSelector::new(5, PrivilegeLevel::Ring3);
    let ss_sysret = SegmentSelector::new(4, PrivilegeLevel::Ring3);

    if let Err(err) = Star::write(cs_sysret, ss_sysret, cs_syscall, ss_syscall) {
        panic!("System Call: Failed to initialize STAR register (Error: {})", err)
    }

    // Set rip for syscall
    LStar::write(VirtAddr::new(syscall_handler as u64));
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
    options(noreturn)
    );
}
#[no_mangle]
pub unsafe extern "C" fn syscall_abort() {
    let syscall_number: u64;

    asm!(
    "mov {}, rax", out(reg) syscall_number
    );

    panic!("System call with id [{}] does not exist!", syscall_number);
}