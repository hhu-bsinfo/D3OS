#![no_std]

use syscall::{syscall, SystemCall};
use core::result::Result::{Err, Ok};
use terminal::{print, println};
use naming::shared_types::{Capability, OpenOptions};

pub fn share_syscall(thread_id: usize, syscall_num: usize) -> bool{
    let res = syscall(SystemCall::ShareSyscallCap, &[thread_id, syscall_num]);
    match res {
        Ok(b) => b == 0,
        Err(_) => false, 
    }
}
pub fn revoke(thread_id: usize, syscall_num: usize) -> isize{
    let res = syscall(SystemCall::RevokeSyscallCap, &[thread_id, syscall_num]);
    match res {
        Ok(b) => b as isize,
        Err(e) => {
            println!("Syscall: Share Naming Cap failed with error {}", e as isize);
            e as isize
        },
    }
}
pub fn share_naming_object(thread_id: usize, rights: OpenOptions, cap: Capability) -> isize{ 
    let res = syscall(SystemCall::ShareNamingCap, &[thread_id, rights.bits(), cap.handle()]);
    match res {
        Ok(b) => b as isize,
        Err(e) => {
            println!("Syscall: Share Naming Cap failed with error {}", e as isize);
            e as isize
        },
    }
}

pub fn revoke_naming_object(thread_id: usize, cap: Capability) -> isize{
    let res = syscall(SystemCall::RevokeNamingCap, &[thread_id, cap.handle()]);
    match res {
        Ok(b) => b as isize,
        Err(e) => {
            println!("Syscall: Revoke Naming Cap failed with error {}", e as isize);
            e as isize
        },
    }
}

pub fn get_naming_len() -> usize {
    let res = syscall(SystemCall::NamingLen, &[]);
    res.unwrap_or_else(|e| {
        println!("Syscall: Get Naming Len failed with error {}", e as isize);
        0
    })
}