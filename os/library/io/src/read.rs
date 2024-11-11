use syscall::{syscall, SystemCall};

use crate::Application;

pub fn read(application: Application) -> char {
    let application_addr = core::ptr::addr_of!(application) as usize;
    let res = syscall(SystemCall::TerminalRead, &[application_addr, 1]);
    
    match res {
        Ok(ch) => char::from_u32(ch as u32).unwrap(),
        Err(_) => panic!("Failed to read from terminal"),
    }
}

pub fn try_read(application: Application) -> Option<char> {
    let application_addr = core::ptr::addr_of!(application) as usize;
    let res = syscall(SystemCall::TerminalRead, &[application_addr, 0]);
    match res {
        Ok(ch) => Some(char::from_u32(ch as u32).unwrap()),
        Err(_) => None,
    }
}
