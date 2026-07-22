use core::ptr;
use core::sync::atomic::{AtomicI16, AtomicU16, Ordering};
use syscall::{SystemCall, return_vals::SyscallResult, syscall};
use uuid::Uuid;

// Schnittstelle für Interprozesskommunikation

pub const MESSAGE_SIZE: usize = 0x100;
const MAX_NUM_MESSAGES: usize = 8; // !this needs to be a power of 2!

pub struct IpcQueue {
    writer_idx: AtomicU16,
    reader_idx: AtomicU16,
    mailbox: [AtomicI16; MAX_NUM_MESSAGES],
    data: [[u8; MESSAGE_SIZE]; MAX_NUM_MESSAGES],
}

impl IpcQueue {
    
    pub fn send(&mut self, data: &[u8]) -> bool {

        // 1. check if there is space in the queue
        if self.space_left() == 0 { return false }

        let writer_idx = self.writer_idx().fetch_add(1, Ordering::SeqCst) as usize % MAX_NUM_MESSAGES;

        // 2. check (and lock) mailbox entry
        match self.mailbox[writer_idx].compare_exchange(0, -1, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => {
                // 3. write message into memory
                let msg_len = core::cmp::min(data.len(), MESSAGE_SIZE);
                self.data[writer_idx][..msg_len].copy_from_slice(&data[..msg_len]);
                self.mailbox[writer_idx].store(msg_len as i16, Ordering::SeqCst);
                true
            }
            Err(_) => false,
        }
    }

    pub fn receive(&'static self) -> Option<IpcMessageData> {

        // 1. check for any messages
        if self.messages_left() == 0 { return None }

        //let writer_idx = self.writer_idx().load(Ordering::SeqCst) as usize % MAX_NUM_MESSAGES;
        let reader_idx = self.reader_idx().fetch_add(1, Ordering::SeqCst) as usize % MAX_NUM_MESSAGES;

        let msg_len = self.mailbox[reader_idx].swap(-1, Ordering::SeqCst);

        // check if reached 'invalid' (i.e. either currently being written, or already read msg)
        if msg_len <= 0 {
            // reset reader idx
            self.reader_idx().store(reader_idx as u16, Ordering::SeqCst);
            return None;
        }

        Some(IpcMessageData { data: &self.data[reader_idx], msg_len: msg_len as usize, queue_ptr: ptr::from_ref(&self.mailbox[reader_idx]) })
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
        Self { writer_idx: AtomicU16::new(0), reader_idx: AtomicU16::new(0), mailbox: [const { AtomicI16::new(0) }; _], data: [[0; MESSAGE_SIZE]; MAX_NUM_MESSAGES] }
    }
}

impl Default for IpcQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// type used for receiving messages, and dropping them after use
pub struct IpcMessageData {
    data: &'static [u8],
    msg_len: usize,
    queue_ptr: *const AtomicI16,
}

impl IpcMessageData {
    pub fn message(&self) -> &[u8] {
        &self.data[..self.msg_len]
    }
}

impl Drop for IpcMessageData {
    fn drop(&mut self) {
        unsafe {
            (*self.queue_ptr).store(0, Ordering::SeqCst);
        }
    }
}

const USER_SPACE_MAILBOX_START: usize = 0x10100000000;

pub fn init_queue() {
    let mailbox_start = USER_SPACE_MAILBOX_START as *const u8;
    let ipc_queue_ptr = mailbox_start as *mut IpcQueue;
    unsafe { 
        ipc_queue_ptr.write(IpcQueue::new());
    }
}

pub fn send(pid: Uuid, tid: usize, message: &[u8]) -> SyscallResult {
    
    let (pid1, pid2) = pid.as_u64_pair();
    let msg_ptr = message.as_ptr();
    let msg_len = message.len();

    syscall(SystemCall::IpcSend, &[pid1 as usize, pid2 as usize, tid, msg_ptr as usize, msg_len])
}

pub fn receive() -> Option<IpcMessageData> {
    let ipc_queue = unsafe { (USER_SPACE_MAILBOX_START as *const IpcQueue).as_ref() };
    ipc_queue.and_then(|iq| iq.receive())
}

pub fn receive_blocking(timeout_ms: usize) -> Result<IpcMessageData, syscall::return_vals::Errno> {
    match syscall(SystemCall::IpcReceiveBlocking, &[timeout_ms]) {
        Ok(_) => receive().ok_or(syscall::return_vals::Errno::EEXIST),
        Err(e) => Err(e),
    }
}