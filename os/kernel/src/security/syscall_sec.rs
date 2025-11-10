use crate::{process::thread::Thread, scheduler};

use super::sec::{load_user_byte, store_user_byte};
use alloc::sync::Arc;
use syscall::return_vals::{Errno, SyscallResult};
use crate::consts::{USER_SPACE_START, USER_SPACE_END};

pub fn access_ok(addr: usize, len: usize) -> SyscallResult {
    let top = addr.wrapping_add(len);
    let valid = addr >= USER_SPACE_START && top <= USER_SPACE_END && top >= addr;

    if valid { Ok(0) } else { Err(Errno::EACCES) }
}


pub fn copy_from_user(dest: *mut u8, src: *const u8, len: usize) -> SyscallResult {
    let current = scheduler().current_thread();
    let t = unsafe { &mut *(Arc::as_ptr(&current) as *mut Thread) };
    t.copy_faulted(0);

    unsafe {
        for i in 0..len {
            if t.faulted().is_err() {
                t.copy_faulted(0); // unmark before returning
                return Err(Errno::EFAULT);
            }
            *dest.add(i) = load_user_byte(src.add(i));
        }
    }

    Ok(0)
}

pub fn copy_to_user(dest: *mut u8, src: *const u8, len: usize) -> SyscallResult {
    let current = scheduler().current_thread();
    let t = unsafe { &mut *(Arc::as_ptr(&current) as *mut Thread) };
    t.copy_faulted(0);

    unsafe {
        for i in 0..len {
            if t.faulted().is_err() {
                t.copy_faulted(0);
                return Err(Errno::EFAULT);
            }
            store_user_byte(dest.add(i), *src.add(i));
        }
    }

    Ok(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn __fixup_copy_from_user() {
    mark_fault_in_thread();
}

#[unsafe(no_mangle)]
pub extern "C" fn __fixup_copy_to_user() {
    mark_fault_in_thread();
}

fn mark_fault_in_thread() {
    let current = scheduler().current_thread();
    let t = unsafe { &mut *(Arc::as_ptr(&current) as *mut Thread) };
    t.copy_faulted(1);
}