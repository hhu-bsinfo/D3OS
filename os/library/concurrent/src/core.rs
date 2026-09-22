use syscall::{syscall, SystemCall};

pub fn current_core_id() -> Option<u32> {
    let res = syscall(SystemCall::CoreId, &[]);
    match res {
        Ok(id) => Some(id as u32),
        Err(_) => None,
    }
}