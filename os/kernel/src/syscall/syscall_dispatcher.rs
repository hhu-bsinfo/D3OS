/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: syscall_dispatcher                                              ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Low-level dispatcher for system calls.                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 25.8.2025, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/



use core::arch::{asm, naked_asm};
use core::mem::size_of;
use core::ops::Deref;
use core::ptr;
use syscall::NUM_SYSCALLS;
use x86_64::registers::control::{Efer, EferFlags};
use x86_64::registers::model_specific::{KernelGsBase, LStar, SFMask, Star};
use x86_64::structures::gdt::SegmentSelector;
use x86_64::{PrivilegeLevel, VirtAddr};

use crate::capabilities::capability::CapabilityFlags;
use crate::{core_local_storage, scheduler, tss};
use log::{error, info};
use x86_64::registers::rflags::RFlags;


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
    info!("Initializing system calls");

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
    LStar::write(VirtAddr::new(syscall_handler as *const () as u64));

    // Make sure interrupts are disabled during system calls
    // The CPU clears every flag that is set in the SFMask register
    SFMask::write(RFlags::INTERRUPT_FLAG);

    // Initialize core local storage (accessible via 'swapgs')
    let mut core_local_storage = core_local_storage().lock();
    core_local_storage.tss_rsp0_ptr = VirtAddr::new(ptr::from_ref(tss().lock().deref()) as u64 + size_of::<u32>() as u64);
    KernelGsBase::write(VirtAddr::new(ptr::from_ref(core_local_storage.deref()) as u64));
}

#[unsafe(naked)]
///
/// Description: \
///    This function does not take any parameters per its declaration,
///    but in reality, it takes at least the system call ID in rax
///    and may take additional parameters for the system call in `rdi`, `rsi` ... \
///    See AMD64 ABI.
///
/// Return: \
///    Two values in `rax`, `rdx` to reconstruct `Result`in user mode
unsafe extern "sysv64" fn syscall_handler() {
    naked_asm!(
    // Switch to kernel stack
    "swapgs", // Setup core local storage access via gs base
    "mov gs:[{CORE_LOCAL_STORAGE_USER_RSP_INDEX}], rsp", // Temporarily store user rip in core local storage
    "mov rsp, gs:[{CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX}]", // Load pointer to rsp0 entry of tss from core local storage
    "mov rsp, [rsp]", // Dereference rsp0 pointer to switch to kernel stack
    "push gs:[{CORE_LOCAL_STORAGE_USER_RSP_INDEX}]", // Store user rip on kernel stack (core local storage might be overwritten, when a thread switch occurs during system call execution)
    "swapgs", // Restore gs base

    // Store registers according to System V AMD64 ABI
    // (except rax, which is used for system call ID and return value)
    "push rcx", // Contains rip for returning to ring 3
    "push rdx",
    "push rdi",
    "push rsi",
    "push r8",
    "push r9",
    "push r10",
    "push r11", // Contains rflags for returning to ring 3
    "push 0", // Push another value so that the stack pointer is 16-byte aligned (needed for u128)

    // copy 4th argument to rcx to adhere System V AMD64 ABI
    "mov rcx, r10",

    // Enable interrupts (we are now on the kernel stack and can handle them properly)
    "sti",

    // Check if system call ID is in bounds
    "cmp rax, {NUM_SYSCALLS}",
    "jge syscall_abort", // Panics and does not return

    //Save all Registers again because get_capability_entry would overwrite them
    //"push rbx",
    "push rcx", // Contains rip for returning to ring 3
    "push rdx",
    "push rdi",
    "push rsi",
    "push r8",
    "push r9",
    "push r10",
    "push r11", // Contains eflags for returning to ring 3
    // "push r12",
    // "push r13",
    // "push r14",
    // "push r15",

    "call get_capability_entry", // Write Syscall Function Address from Corresponding Capability to stack

    //Restore registers to use them in the syscall function
    // "pop r15",
    // "pop r14",
    // "pop r13",
    // "pop r12",
    "pop r11", // Contains eflags for returning to ring 3
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rsi",
    "pop rdi",
    "pop rdx",
    "pop rcx", // Contains rip for returning to ring 3
    //"pop rbx",

    "call rax", // Call system call function pointer

    // Restore registers
    // "pop r15", // the 0 from above
    // "pop r15",
    // "pop r14",
    // "pop r13",
    // "pop r12",

    "pop r11", // Pop the alignment 0
    "pop r11", // Contains rflags for returning to ring 3
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rsi",
    "pop rdi",
    "pop rdx",
    "pop rcx", // Contains rip for returning to ring 3

    // Switch back to user stack
    "cli", // Disable interrupts, since we are still in Ring 0 and no interrupt handler should be called with the user stack
    "pop rsp", // Restore rsp from kernel stack

    // Return to Ring 3
    // Interrupts will be enabled automatically, because rflags is restored from r11
    "sysretq",
    NUM_SYSCALLS = const NUM_SYSCALLS,
    CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX = const CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX,
    CORE_LOCAL_STORAGE_USER_RSP_INDEX = const CORE_LOCAL_STORAGE_USER_RSP_INDEX,
    );
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "C" fn syscall_abort() {
    naked_asm!(
        "mov rdi, rax", // Move syscall number to first argument (rdi)
        "call syscall_abort_panic"
    )
}

#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_abort_panic(syscall_number: u64) {
    panic!("System call with id [{}] does not exist!", syscall_number);
}

#[unsafe(no_mangle)]
extern "sysv64" fn permission_denied() -> isize {
    syscall::return_vals::Errno::EACCES as isize
}

///Gets the function pointer of the syscall with the given ID from the capability
#[unsafe(no_mangle)]
unsafe extern "C" fn get_capability_entry() -> *const () {
    let syscall_number: u64;
    unsafe {
        asm!("mov {}, rax", out(reg) syscall_number);
    }
    // info!("Syscall number: {}", syscall_number);
    // Get current thread's CSpace through scheduler
    let current_thread = scheduler().current_thread();

    //info!("cspace obj is locked: {}", current_thread.cspace.is_locked());

    // Check capability and return function pointer if allowed
    let pointer: *const () = {
        if let Some(cspace) = current_thread.cspace.invoke() {
            if let Some(syscall_cap) = cspace.get_syscall_capability(syscall_number as usize) {
                if syscall_cap.has_permissions(CapabilityFlags::EXECUTE)
                    && let Some(syscall) = syscall_cap.invoke()
                {
                    // Get the syscall function pointer from the capability
                    syscall.function_pointer()
                } else {
                    error!("Syscall capability for syscall id [{}] does not have EXECUTE permission!", syscall_number);
                    permission_denied as *const ()
                }
            } else {
                error!("Syscall capability for syscall id [{}] does not exist!", syscall_number);
                permission_denied as *const ()
            }
        } else {
            error!(
                "Could not invoke CSpace for current thread when trying to get syscall capability for syscall id [{}]!, thread: {}",
                syscall_number,
                scheduler().current_ids().1
            );
            permission_denied as *const ()
        }
    };

    // Check if the syscall function pointer is valid
    if !pointer.is_null() {
        // Store the function pointer in rax for syscall_disp to call
        return pointer;
    }

    // If we get here, something went wrong
    error!("Could not get syscall function pointer for syscall id [{}]!", syscall_number);
    permission_denied as *const ()
    //panic!("Capability for syscall with id [{}] does not exist or has no permission!", syscall_number);
}