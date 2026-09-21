use syscall::return_vals::{self, Errno};
use uuid::Uuid;

use crate::ipc_message::IpcMessageWrapper;

pub mod ipc_customqueue;
pub mod ipc_lfqueue;
pub mod buffer_queue;
pub mod ipc_lockqueue;
pub mod atomic_ringbuffer;
pub mod ipc_ringbufferqueue;
//pub mod ipc_lfdsqueue;


pub type IpcQueueType = ipc_ringbufferqueue::IpcRingbufferQueue<crate::ipc_message::IpcMessage, { crate::MAX_NUM_MESSAGES }>;
//pub type IpcQueueType = ipc_lfqueue::IpcLFQueue;

pub trait IpcQueue {
    fn new() -> Self;
    fn send(&self, name: &str, msg_type: &str, data: &[u8], sender_pid: &Uuid, sender_tid: usize) -> Result<usize, IpcError>;
    fn receive(&self) -> Result<impl IpcMessageWrapper, IpcError>;
    fn messages_left(&self) -> usize;
}

#[derive(Debug, PartialEq)]
pub enum IpcError {
    Generic = -1,
    Timeout = -2,
    Full = -3,
    Empty = -4,
    MsgTooLarge = -5,
    NoIpcQueue = -6,
    Invalid = -7,
}

impl IpcError {
    pub fn convert_syscall_result(syscall_result: Result<usize, Errno>) -> Result<usize, Self> {
        match syscall_result {
            Ok(ok) => Ok(ok),
            Err(err) => Err(err.into()),
        }
    }
}

impl From<IpcError> for isize {
    fn from(value: IpcError) -> Self {
        value as isize
    }
}

impl From<isize> for IpcError {
    fn from(value: isize) -> Self {
        match value {
            -1 => IpcError::Generic,
            -2 => IpcError::Timeout,
            -3 => IpcError::Full,
            -4 => IpcError::Empty,
            -5 => IpcError::MsgTooLarge,
            -6 => IpcError::NoIpcQueue,
            -7 => IpcError::Invalid,
            _ => IpcError::Generic,
        }
    }
}

impl From<Errno> for IpcError {
    fn from(value: Errno) -> Self {
        let ret_code = return_vals::convert_syscall_result_to_ret_code(Err(value));
        ret_code.into()
    }
}