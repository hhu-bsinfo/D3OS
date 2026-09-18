use core::sync::atomic::Atomic;
use core::{cell::UnsafeCell, mem::MaybeUninit};

use lfqueue::{BoundedQueue, ConstBoundedQueue, const_queue};
use crossbeam_utils::CachePadded;

use crate::{MAX_NUM_MESSAGES, MESSAGE_DATA_SIZE};
use crate::ipc_message::{IpcMessage, IpcMessageWrapper};

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

    fn send(&self, name: &str, msg_type: &str, message: &[u8], id: &uuid::Uuid, sender_tid: usize) -> bool {
        let mut data = [0; MESSAGE_DATA_SIZE];
        let l = core::cmp::min(message.len(), MESSAGE_DATA_SIZE);
        data[..l].copy_from_slice(&message[..l]);
        self.queue.enqueue(IpcMessage { name: name.into(), msg_type: msg_type.into(), data, data_len: message.len(), sender_pid: *id, sender_tid }).is_ok()
    }
    
    fn receive(&self) -> Option<impl IpcMessageWrapper> {
        Some(IpcMessageWrapperOwned { msg: self.queue.dequeue()? })
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