use syscall::{SystemCall, syscall};
use uuid::Uuid;

// Schnittstelle für Interprozesskommunikation

enum IpcMessageType {
    IpcMessageTypeSmall,
}

struct IpcMessageAddr {
    addr: u16,
    message_type: IpcMessageType
}


pub fn send_small(target_pid: &Uuid, message: usize) -> Result<usize, syscall::return_vals::Errno> {
    let (pid1, pid2) = target_pid.as_u64_pair();
    syscall(SystemCall::IpcSend, &[pid1, pid2, usize])
}

pub fn receive<T: IpcMessage>() -> Result<T, syscall::return_vals::Errno> {
    let res = syscall(SystemCall::IpcReceive, message.collapse());
    match res {
        Some(addr) => Ok(()),
        Err(e) => e,
    }
}

fn store_message_small(message: usize) -> IpcMessageAddr {
    
}