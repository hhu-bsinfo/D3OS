use core::sync::atomic::Atomic;
use core::{cell::UnsafeCell, mem::MaybeUninit};

use lockfree_datastructures::datastructures::hazardpointers::hazard_pointers::HPRecType;
use lockfree_datastructures::datastructures::hazardpointers::hp_lock_free_queue::{LockFreeQueue, NodeType};

use crate::{MAX_NUM_MESSAGES, MESSAGE_DATA_SIZE};
use crate::ipc_message::{IpcMessage, IpcMessageWrapper};

/// IPC Queue using lockfree-datastructures crate
pub struct IpcLFDSQueue {
    queue: LockFreeQueue<IpcMessage>,
    hp: *mut HPRecType<NodeType<IpcMessage>>,
}

#[allow(clippy::unused_unit)]
impl IpcLFDSQueue {
    pub fn new() -> Self {
        Self { queue: LockFreeQueue::new(), hp: LockFreeQueue::<IpcMessage>::allocate_hprec() }
    }
}

impl super::IpcQueue for IpcLFDSQueue {

    fn new() -> Self {
        Self::new()
    }

    fn send(&self, name: &str, msg_type: &str, message: &[u8], id: &uuid::Uuid, sender_tid: usize) -> bool {
        let mut data = [0; MESSAGE_DATA_SIZE];
        let l = core::cmp::min(message.len(), MESSAGE_DATA_SIZE);
        data[..l].copy_from_slice(&message[..l]);
        self.queue.enqueue(
            IpcMessage { name: name.into(), msg_type: msg_type.into(), data, data_len: message.len(), sender_pid: *id, sender_tid },
            self.hp
        );
        true
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