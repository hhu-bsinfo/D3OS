use uuid::Uuid;

use crate::ipc_message::{IpcMessage, IpcMessageType, IpcMessageWrapper};
use crate::ipc_error::IpcError;

#[cfg(feature = "lfqueue")]
pub mod ipc_lfqueue;

#[cfg(feature = "ringbuffer")]
pub mod atomic_ringbuffer;
#[cfg(feature = "ringbuffer")]
pub mod ipc_ringbuffer;

#[cfg(feature = "ringqueue")]
pub mod semiatomic_ringqueue;
#[cfg(feature = "ringqueue")]
pub mod ipc_ringqueue;
#[cfg(feature = "ringqueue")]
pub mod atomic_ringbuffer;

pub type IpcQueueType = cfg_select! {
    feature = "lfqueue" => {ipc_lfqueue::IpcLFQueue}
    feature = "ringbuffer" => {ipc_ringbuffer::IpcRingbuffer<crate::ipc_message::IpcMessage, { crate::MAX_NUM_MESSAGES }>}
    feature = "ringqueue" => {ipc_ringqueue::IpcRingQueue<crate::ipc_message::IpcMessage, { crate::MAX_NUM_MESSAGES }>}
};

pub trait IpcQueue {
    fn new() -> Self;
    fn send(&self, name: &str, bytes: &[u8], msg_type: IpcMessageType, sender_pid: &Uuid, sender_tid: usize, callback: &str) -> Result<usize, IpcError>;
    fn receive(&self) -> Result<impl IpcMessageWrapper, IpcError>;
    fn receive_blocking(&self, timeout_ms: usize) -> Result<impl IpcMessageWrapper, IpcError>;
    fn messages_left(&self) -> usize;
    /// unblock_fn declares the function which the wait_queue should use to unblock any waiters \
    /// this is abstraction is used to avoid calling the scheduler directly, as this can only be done from the kernel;
    /// this solution avoids the direct dependency of this library on the D3OS kernel
    fn unblock_waiting<F: Fn(Uuid, usize) -> bool>(&self, unblock_fn: F);
}

/// message wrapper which takes ownership of the message (i.e. copies the message)
pub struct IpcMessageWrapperOwned {
    pub msg: IpcMessage,
}

impl IpcMessageWrapper for IpcMessageWrapperOwned {
    fn msg(&self) -> &IpcMessage {
        &self.msg
    }
}

impl core::fmt::Display for IpcMessageWrapperOwned {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

pub struct YieldD3OS;

impl spin::RelaxStrategy for YieldD3OS {
    fn relax() {
        concurrent::thread::switch();
    }
}