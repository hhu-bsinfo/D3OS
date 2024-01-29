use syscall::{syscall0, SystemCall};

pub fn read() -> char {
    char::from_u32(syscall0(SystemCall::Read as u64) as u32).unwrap()
}