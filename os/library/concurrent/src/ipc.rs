use core::{ptr, sync::atomic::AtomicU64};

use alloc::boxed::Box;
use spin::Mutex;
use syscall::{SystemCall, return_vals::SyscallResult, syscall};
use uuid::Uuid;
use nolock::queues::mpsc::jiffy;

// Schnittstelle für Interprozesskommunikation

#[derive(Debug, Copy, Clone)]
pub enum IpcMessage {
    Simple {
        a: usize,
        b: usize,
        c: usize,
        d: usize,
    }
}

impl IpcMessage {
    pub fn from_arr(arr: &[usize]) -> IpcMessage {
        let a = arr.get(0).copied().unwrap_or_default();
        let b = arr.get(1).copied().unwrap_or_default();
        let c = arr.get(2).copied().unwrap_or_default();
        let d = arr.get(3).copied().unwrap_or_default();
        Self::Simple { a, b, c, d }
    }
}

const USER_SPACE_MAILBOX_START: usize = 0x10100000000;

// TODO dont store messages themselves in the queue, instead store pointers to message ==> avoid one copy (currently 2-copy)
// TODO far future: use rdma to allow zero-copy ==> store large data structures in specific pages; when sending the message, unmap the page and remap into receiver

pub fn init_queue() {
    let (receiver, sender) = jiffy::queue();
    let handles = Box::new(IpcQueueHandles { sender, receiver: Mutex::new(receiver), num_messages: AtomicU64::new(0) });
    let handles_ptr = ptr::from_ref(Box::as_ref(&handles)); 
    let handles_ptr_dest = USER_SPACE_MAILBOX_START as *mut *const IpcQueueHandles;
    unsafe { handles_ptr_dest.write(handles_ptr); }
}

pub fn send(pid: Uuid, tid: usize, message: &IpcMessage) -> SyscallResult {
    
    let (pid1, pid2) = pid.as_u64_pair();
    let message_ptr = ptr::from_ref(message);

    syscall(SystemCall::IpcSend, &[pid1 as usize, pid2 as usize, tid, message_ptr as usize])
}

pub fn receive_next() -> Option<IpcMessage> {
    let handles = unsafe { &*(USER_SPACE_MAILBOX_START as *const IpcQueueHandles) };
    handles.receiver.lock().dequeue()
}

pub fn receive_next_blocking(timeout_ms: usize) -> Result<IpcMessage, syscall::return_vals::Errno> {
    match syscall(SystemCall::IpcReceiveBlocking, &[timeout_ms]) {
        Ok(_) => {
            let handles = unsafe { &*(USER_SPACE_MAILBOX_START as *const IpcQueueHandles) };
            handles.receiver.lock().dequeue().ok_or(syscall::return_vals::Errno::EEXIST)
        },
        Err(e) => Err(e),
    }
}