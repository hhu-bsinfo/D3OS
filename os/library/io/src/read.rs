use syscall::{syscall, SystemCall};

use crate::Application;

pub fn read(application: Application) -> char {
    let application_addr = core::ptr::addr_of!(application) as usize;
    char::from_u32(syscall(SystemCall::Read, &[application_addr, 1]).expect("Read Syscall failed") as u32).unwrap()
}

pub fn try_read(application: Application) -> Option<char> {
    let application_addr = core::ptr::addr_of!(application) as usize;
    let character = syscall(SystemCall::Read, &[application_addr, 0]).expect("Read Syscall failed") as u32;
    match character {
        0 => None,
        u32_char @ 1.. => Some(char::from_u32(u32_char).unwrap()),
    }
}
