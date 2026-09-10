use core::sync::atomic::{AtomicI16, AtomicU16, Ordering};
use uuid::Uuid;

use crate::{MAX_NUM_MESSAGES, MESSAGE_DATA_SIZE};
use crate::ipc_message::{IpcMessage, IpcMessageWrapper};


#[derive(Debug)]
pub struct IpcQueue {
    writer_idx: AtomicU16,
    reader_idx: AtomicU16,
    mailbox: [AtomicI16; MAX_NUM_MESSAGES],
    messages: [IpcMessage; MAX_NUM_MESSAGES],
}

impl IpcQueue {
    
    pub fn send(&self, name: &str, msg_type: &str, data: &[u8], sender_pid: &Uuid) -> bool {

        // 1. increment writer idx
        let writer_idx = self.writer_idx().fetch_add(1, Ordering::SeqCst) as usize % MAX_NUM_MESSAGES;

        // 2. check (and lock) mailbox entry
        if self.mailbox[writer_idx].compare_exchange(0, -1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            // 3. get msg as mut
            //    -> this is sound because we ensure we are the sole thread accessing this message, using the atomic variable above
            if let Some(msg) = self.messages.get(writer_idx).and_then(|msg| unsafe { core::ptr::from_ref(msg).cast_mut().as_mut() }) {
                // 4. write message
                msg.name.set(name);
                msg.msg_type.set(msg_type);
                let msg_len = core::cmp::min(data.len(), MESSAGE_DATA_SIZE);
                msg.data[..msg_len].copy_from_slice(&data[..msg_len]);
                msg.data_len = msg_len;
                msg.sender_pid = *sender_pid;
                self.mailbox[writer_idx].store(msg_len as i16, Ordering::SeqCst);
                return true;
            }
        }

        self.writer_idx().store(writer_idx as u16, Ordering::SeqCst);
        false
    }

    pub fn receive(&'static self) -> Option<IpcMessageWrapper> {

        // iter until writer_idx to check for any new messages
        let reader_idx = self.reader_idx().fetch_add(1, Ordering::SeqCst) as usize % MAX_NUM_MESSAGES;
        
        // lock mailbox slot
        let msg_len = self.mailbox[reader_idx].swap(-1, Ordering::SeqCst);

        // check if found valid message
        if msg_len > 0 {
            // put Message into Wrapper to enable automatic clearing of the queue, once the value is dropped
            self.messages.get(reader_idx).map(|msg| IpcMessageWrapper { msg, queue_ptr: &self.mailbox[reader_idx] })
        } else {
            // free mailbox slot again
            self.mailbox[reader_idx].store(msg_len, Ordering::SeqCst);
            // reset reader idx
            self.reader_idx().store(reader_idx as u16, Ordering::SeqCst);
            None
        }
    }

    pub fn writer_idx(&self) -> &AtomicU16 {
        &self.writer_idx
    }

    pub fn reader_idx(&self) -> &AtomicU16 {
        &self.reader_idx
    }

    pub fn messages_left(&self) -> usize {
        self.mailbox.iter().filter(|a| a.load(Ordering::SeqCst) > 0).count()
    }

    pub fn space_left(&self) -> usize {
        MAX_NUM_MESSAGES - self.messages_left()
    }

    pub fn new() -> Self {
        Self { writer_idx: AtomicU16::new(0), reader_idx: AtomicU16::new(0), mailbox: [const { AtomicI16::new(0) }; MAX_NUM_MESSAGES], messages: [IpcMessage::new(); MAX_NUM_MESSAGES] }
    }
}

impl Default for IpcQueue {
    fn default() -> Self {
        Self::new()
    }
}