use spin::Mutex;

use crate::queues::{IpcError, IpcQueue};
use crate::queues::atomic_ringbuffer::Ringbuffer;
use crate::ipc_message::{IpcMessage, IpcMessageType, IpcMessageWrapper};
use crate::receive;


pub struct IpcRingbufferQueue<T, const S: usize> {
    ringbuffer: Ringbuffer<T, S>,
    waiting_threads: Mutex<[Option<(uuid::Uuid, usize)>; S]>
}

impl<const S: usize> IpcQueue for IpcRingbufferQueue<IpcMessage, S> {
    fn new() -> Self {
        Self { ringbuffer: Ringbuffer::new(), waiting_threads: Mutex::new([const { None }; _]) }
    }
    
    fn send(&self, name: &str, bytes: &[u8], msg_type: IpcMessageType, sender_pid: &uuid::Uuid, sender_tid: usize) -> Result<usize, IpcError> {

        // reserve space in ringbuffer
        let reserved = self.ringbuffer.reserve();
        if let Some((ptr, _guard)) = reserved {

            // write data into msg
            let msg = unsafe { &mut (*ptr) };
            msg.set_from_data_bytes(name, bytes, msg_type, *sender_pid, sender_tid);
            
            Ok(0)
        } else {
            Err(IpcError::Full)
        }
    }

    fn receive(&self) -> Result<impl IpcMessageWrapper, IpcError> {
        self.ringbuffer.pop().map(|msg| IpcMessageWrapperOwned { msg }).ok_or(IpcError::Empty)
    }

    fn receive_blocking(&self, timeout_ms: usize) -> Result<impl IpcMessageWrapper, IpcError> {

        if self.messages_left() > 0 {
            return self.receive();
        }

        let timeout_time = syscall::syscall(syscall::SystemCall::GetSystemTime, &[]).unwrap() + timeout_ms;
        let free = self.waiting_threads.lock().iter().position(|opt| opt.is_none());
        if let Some(idx) = free {
            loop {
                if self.messages_left() > 0 {
                    return self.receive();
                }
                if syscall::syscall(syscall::SystemCall::GetSystemTime, &[]).unwrap() >= timeout_time {
                    return Err(IpcError::Timeout);
                }

                let mut guard = self.waiting_threads.lock();
                
                let pid = concurrent::process::current().id();
                let tid = concurrent::thread::current().unwrap().id();
                guard[idx] = Some((pid, tid));
                drop(guard);
                concurrent::thread::sleep(timeout_ms);
            }
        }

        Err(IpcError::Empty)
    }

    fn messages_left(&self) -> usize {
        self.ringbuffer.num_elements()
    }
    
    fn unblock_waiting<F: Fn(uuid::Uuid, usize) -> bool>(&self, unblock_fn: F) {
        let mut guard = self.waiting_threads.lock();
        for opt in guard.iter_mut() {
            if let Some((pid, tid)) = opt {
                let unblock_success = unblock_fn(*pid, *tid);
                if unblock_success {
                    *opt = None;
                }
                // TODO when unblock fails here: thread was registered as waiting but missed unblock
            }
        }
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