use core::sync::atomic::Atomic;
use core::{cell::UnsafeCell, mem::MaybeUninit};

use lfqueue::{BoundedQueue, ConstBoundedQueue, const_queue};
use crossbeam_utils::CachePadded;

use crate::queues::IpcError;
use crate::MAX_NUM_MESSAGES;
use crate::ipc_message::{IpcMessage, IpcMessageContent, IpcMessageData, IpcMessageWrapper};

type BackingStorage = [UnsafeCell<MaybeUninit<IpcMessage>>; MAX_NUM_MESSAGES * 2];
type Ring = [CachePadded<Atomic<usize>>; MAX_NUM_MESSAGES * 2];

/// IPC Queue using lfqueue crate
/// 
/// should be more robust than the custom IpcQueue implementation, however the library copies on read, meaning the whole operation requires two copies
pub struct IpcLFQueue {
    queue: BoundedQueue<IpcMessage, BackingStorage, Ring, 0>
}

#[allow(clippy::unused_unit)]
impl IpcLFQueue {
    pub fn new() -> Self {
        let cq = const_queue!(IpcMessage; MAX_NUM_MESSAGES);
        Self { queue: cq }
    }
}

impl super::IpcQueue for IpcLFQueue {

    fn new() -> Self {
        Self::new()
    }

    fn send(&self, name: &str, bytes: &[u8], sender_pid: &uuid::Uuid, sender_tid: usize) -> Result<usize, IpcError> {
        let msg = IpcMessage::new_from(name, bytes.to_data(), *sender_pid, sender_tid);
        let enq = self.queue.enqueue(msg);
        match enq {
            Ok(_) => Ok(0),
            Err(_) => Err(IpcError::Generic),
        }
    }
    
    fn receive(&self) -> Result<impl IpcMessageWrapper, IpcError> {
        Ok(IpcMessageWrapperOwned { msg: self.queue.dequeue().ok_or(IpcError::Generic)? })
    }
    
    fn messages_left(&self) -> usize {
        todo!()
    }
}

pub struct IpcMessageWrapperOwned {
    pub msg: IpcMessage,
}

impl IpcMessageWrapper for IpcMessageWrapperOwned {
    fn msg(&self) -> &IpcMessage {
        &self.msg
    }
}

impl core::ops::Deref for IpcMessageWrapperOwned {
    type Target = IpcMessage;

    fn deref(&self) -> &Self::Target {
        &self.msg
    }
}

impl core::fmt::Display for IpcMessageWrapperOwned {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.msg)
    }
}