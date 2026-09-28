use spin::RwLock;

use crate::queues::IpcError;
use crate::queues::buffer_queue::BufferQueue;
use crate::queues::ipc_lfqueue::IpcMessageWrapperOwned;
use crate::MAX_NUM_MESSAGES;
use crate::ipc_message::{IpcMessage, IpcMessageContent, IpcMessageData, IpcMessageType, IpcMessageWrapper};

pub struct IpcLockQueue {
    messages: RwLock<BufferQueue<IpcMessage, MAX_NUM_MESSAGES>>,
}

impl super::IpcQueue for IpcLockQueue {

    fn new() -> Self {
        Self { messages: RwLock::new(BufferQueue::new()) }
    }

    fn send(&self, name: &str, bytes: &[u8], _msg_type: IpcMessageType, sender_pid: &uuid::Uuid, sender_tid: usize) -> Result<usize, IpcError> {
        let msg = IpcMessage::new_from(name, bytes.to_data(), *sender_pid, sender_tid);
        self.messages.write().enqueue(msg)
    }

    fn receive(&self) -> Result<impl IpcMessageWrapper, IpcError> {
        Ok(IpcMessageWrapperOwned {
            msg: self.messages.write().dequeue_front().ok_or(IpcError::Full)?
        })
    }

    fn messages_left(&self) -> usize {
        self.messages.read().num_objects()
    }
}

pub struct IpcMessageWrapperBuffer<'a> {
    pub msg: &'a IpcMessage,
    pub queue_ref: &'a IpcLockQueue,
    pub idx: usize,
}

impl<'a> IpcMessageWrapper for IpcMessageWrapperBuffer<'a> {
    fn msg(&self) -> &IpcMessage {
        self.msg
    }
}

impl<'a> Drop for IpcMessageWrapperBuffer<'a> {
    fn drop(&mut self) {
        // this msg still lives in the buffer queue
        // we need to dequeue here
        self.queue_ref.messages.write().dequeue(self.idx);
    }
}