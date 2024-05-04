use syscall::{syscall1, SystemCall};

use crate::Application;

pub fn read(application: Application) -> char {
    let application_addr = core::ptr::addr_of!(application) as usize;
    char::from_u32(syscall1(SystemCall::Read, application_addr) as u32).unwrap()
}