use crate::{ipc_message::{IpcMessage, IpcMessageWrapper}, queues::{IpcQueue, atomic_ringbuffer::Ringbuffer, ipc_lfqueue::IpcMessageWrapperOwned}};


pub struct IpcRingbufferQueue<T, const S: usize> {
    ringbuffer: Ringbuffer<T, S>
}

impl<const S: usize> IpcQueue for IpcRingbufferQueue<IpcMessage, S> {
    fn new() -> Self {
        Self { ringbuffer: Ringbuffer::new() }
    }

    fn send(&self, name: &str, msg_type: &str, data: &[u8], sender_pid: &uuid::Uuid) -> bool {

        // reserve space in ringbuffer
        let reserved = self.ringbuffer.reserve();

        if let Some(ptr) = reserved {

            // get mut ref to msg -> this should be safe, the ringbuffer ensures we are the only writer
            let msg = unsafe { ptr.as_mut_unchecked() };

            // write data into msg
            msg.name.set(name);
            msg.msg_type.set(msg_type);
            let l = core::cmp::min(S, data.len());
            msg.data[..l].copy_from_slice(&data[..l]);
            msg.data_len = l;
            msg.sender_pid = *sender_pid;
            
            true
        } else {
            false
        }
    }

    fn receive(&self) -> Option<impl IpcMessageWrapper> {
        self.ringbuffer.pop().map(|msg| IpcMessageWrapperOwned { msg })
    }

    fn messages_left(&self) -> usize {
        self.ringbuffer.num_elements()
    }
}