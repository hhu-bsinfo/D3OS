use crate::queues::{IpcError, IpcQueue};
use crate::queues::atomic_ringbuffer::Ringbuffer;
use crate::ipc_message::{IpcMessage, IpcMessageWrapper};


pub struct IpcRingbufferQueue<T, const S: usize> {
    ringbuffer: Ringbuffer<T, S>
}

impl<const S: usize> IpcQueue for IpcRingbufferQueue<IpcMessage, S> {
    fn new() -> Self {
        Self { ringbuffer: Ringbuffer::new() }
    }

    fn send(&self, name: &str, msg_type: &str, data: &[u8], sender_pid: &uuid::Uuid, sender_tid: usize) -> Result<usize, IpcError> {

        // reserve space in ringbuffer
        let reserved = self.ringbuffer.reserve();
        if let Some((ptr, _guard)) = reserved {

            // write data into msg
            unsafe {
                (*ptr).name.set(name);
                (*ptr).msg_type.set(msg_type);
                let l = core::cmp::min(S, data.len());
                core::ptr::copy_nonoverlapping(data.as_ptr(), (*ptr).data.as_mut_ptr(), l);
                (*ptr).data_len = l;
                (*ptr).sender_pid = *sender_pid;
                (*ptr).sender_tid = sender_tid;
            }
            
            Ok(0)
        } else {
            Err(IpcError::Full)
        }
    }

    fn receive(&self) -> Result<impl IpcMessageWrapper, IpcError> {
        self.ringbuffer.pop().map(|msg| IpcMessageWrapperOwned { msg }).ok_or(IpcError::Empty)
    }

    fn messages_left(&self) -> usize {
        self.ringbuffer.num_elements()
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

impl core::fmt::Display for IpcMessageWrapperOwned {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.msg)
    }
}