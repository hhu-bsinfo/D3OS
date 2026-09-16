use uuid::Uuid;

use crate::ipc_message::IpcMessageWrapper;

pub mod ipc_customqueue;
pub mod ipc_lfqueue;
pub mod buffer_queue;
pub mod ipc_lockqueue;


pub type IpcQueueType = ipc_lockqueue::IpcLockQueue;

pub trait IpcQueue {
    fn new() -> Self;
    fn send(&self, name: &str, msg_type: &str, data: &[u8], sender_pid: &Uuid) -> bool;
    fn receive(&self) -> Option<impl IpcMessageWrapper>;
    fn messages_left(&self) -> usize;
}
