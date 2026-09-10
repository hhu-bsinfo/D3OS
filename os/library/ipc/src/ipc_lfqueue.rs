use core::sync::atomic::Atomic;
use core::{cell::UnsafeCell, mem::MaybeUninit};

use lfqueue::{BoundedQueue, ConstBoundedQueue, const_queue};
use crossbeam_utils::CachePadded;

use crate::{MAX_NUM_MESSAGES, MESSAGE_DATA_SIZE};
use crate::ipc_message::IpcMessage;

type BackingStorage = [UnsafeCell<MaybeUninit<IpcMessage>>; MAX_NUM_MESSAGES * 2];
type Ring = [CachePadded<Atomic<usize>>; MAX_NUM_MESSAGES * 2];

pub struct IpcLFQueue {
    queue: BoundedQueue<IpcMessage, BackingStorage, Ring, 0>
}

#[allow(clippy::new_without_default)]
#[allow(clippy::unused_unit)]
impl IpcLFQueue {
    pub fn new() -> Self {
        let cq = const_queue!(IpcMessage; MAX_NUM_MESSAGES);
        Self { queue: cq }
    }
    
    pub fn send(&self, name: &str, msg_type: &str, message: &[u8], id: &uuid::Uuid) -> bool {
        let mut data = [0; MESSAGE_DATA_SIZE];
        let l = core::cmp::min(message.len(), MESSAGE_DATA_SIZE);
        data[..l].copy_from_slice(&message[..l]);
        self.queue.enqueue(IpcMessage { name: name.into(), msg_type: msg_type.into(), data, data_len: message.len(), sender_pid: *id, sender_tid: 0 }).is_ok()
    }
    
    pub fn receive(&self) -> Option<IpcMessage> {
        self.queue.dequeue()
    }
    
    pub fn messages_left(&self) -> usize {
        1
    }
}