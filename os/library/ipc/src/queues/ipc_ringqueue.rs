use spin::Mutex;

use crate::queues::semiatomic_ringqueue::{Ringqueue, RingqueueItemWrapper};
use crate::queues::{IpcError, IpcQueue};
use crate::ipc_message::{IpcMessage, IpcMessageType, IpcMessageWrapper};


pub struct IpcRingQueue<T, const S: usize> {
    ringqueue: Ringqueue<T, S>,
    waiting_threads: Mutex<[Option<(uuid::Uuid, usize)>; S]>
}

impl<const S: usize> IpcQueue for IpcRingQueue<IpcMessage, S> {
    fn new() -> Self {
        Self { ringqueue: Ringqueue::new(), waiting_threads: Mutex::new([const { None }; _]) }
    }
    
    fn send(&self, name: &str, bytes: &[u8], msg_type: IpcMessageType, sender_pid: &uuid::Uuid, sender_tid: usize, callback: &str) -> Result<usize, IpcError> {

        // reserve space in ringbuffer
        let reserved = self.ringqueue.reserve();
        if let Some((ptr, _guard)) = reserved {

            // write data into msg
            let msg = unsafe { &mut (*ptr) };
            msg.set_from_data_bytes(name, bytes, msg_type, *sender_pid, sender_tid, callback);
            
            Ok(0)
        } else {
            Err(IpcError::Full)
        }
    }

    fn receive(&self) -> Result<impl IpcMessageWrapper, IpcError> {
        self.ringqueue.pop_ref().ok_or(IpcError::Empty)
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
        self.ringqueue.num_used()
    }
    
    fn unblock_waiting<F: Fn(uuid::Uuid, usize) -> bool>(&self, unblock_fn: F) {
        let mut guard = self.waiting_threads.lock();
        // debug
        let num_waiting = guard.iter().filter(|opt| opt.is_some()).count();
        if num_waiting > 0 {
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
}

impl<'a, const S: usize> IpcMessageWrapper for RingqueueItemWrapper<'a, IpcMessage, S> {
    fn msg(&self) -> &IpcMessage {
        self.read().unwrap()
    }
}