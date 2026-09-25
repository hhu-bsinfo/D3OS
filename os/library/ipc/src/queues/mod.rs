use uuid::Uuid;

use crate::ipc_message::IpcMessageWrapper;
use crate::ipc_error::IpcError;

#[cfg(feature = "customqueue")]
pub mod ipc_customqueue;

#[cfg(feature = "lockqueue")]
pub mod buffer_queue;
#[cfg(feature = "lockqueue")]
pub mod ipc_lockqueue;

#[cfg(feature = "lfqueue")]
pub mod ipc_lfqueue;

#[cfg(feature = "ringbufferqueue")]
pub mod atomic_ringbuffer;
#[cfg(feature = "ringbufferqueue")]
pub mod ipc_ringbufferqueue;

pub type IpcQueueType = cfg_select! {
    feature = "customqueue" => {ipc_customqueue::IpcCustomQueue}
    feature = "lockqueue" => {ipc_lockqueue::IpcLockQueue}
    feature = "lfqueue" => {ipc_lfqueue::IpcLFQueue}
    feature = "ringbufferqueue" => {ipc_ringbufferqueue::IpcRingbufferQueue<crate::ipc_message::IpcMessage, { crate::MAX_NUM_MESSAGES }>}
};

pub trait IpcQueue {
    fn new() -> Self;
    fn send(&self, name: &str, bytes: &[u8], sender_pid: &Uuid, sender_tid: usize) -> Result<usize, IpcError>;
    fn receive(&self) -> Result<impl IpcMessageWrapper, IpcError>;
    fn messages_left(&self) -> usize;
}