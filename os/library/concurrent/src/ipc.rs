use core::fmt::{Debug, Display};
use core::ops::Deref;
use core::sync::atomic::{AtomicI16, AtomicU16, Ordering};
use syscall::{SystemCall, return_vals::SyscallResult, syscall};
use uuid::Uuid;

// Schnittstelle für Interprozesskommunikation

pub struct IpcSyscallParams {
    pub pid: Uuid,
    pub tid: usize,
    pub data_ptr: *const u8,
    pub data_len: usize,
    pub tag_ptr: *const u8,
    pub tag_len: usize,
}

const MESSAGE_TAG_LEN: usize = 0x10;
pub const MESSAGE_SIZE: usize = 0x100 - (size_of::<Uuid>() + size_of::<usize>() + MESSAGE_TAG_LEN);
const MAX_NUM_MESSAGES: usize = 8; // !this needs to be a power of 2!

pub struct IpcQueue {
    writer_idx: AtomicU16,
    reader_idx: AtomicU16,
    mailbox: [AtomicI16; MAX_NUM_MESSAGES],
    data: [IpcMessage; MAX_NUM_MESSAGES],
}

impl IpcQueue {
    
    pub fn send(&mut self, data: &[u8], sender_pid: Uuid, sender_tid: usize, tag: &str) -> bool {

        // 1. check if there is space in the queue
        if self.space_left() == 0 { return false }

        let writer_idx = self.writer_idx().fetch_add(1, Ordering::SeqCst) as usize % MAX_NUM_MESSAGES;

        // 2. check (and lock) mailbox entry
        match self.mailbox[writer_idx].compare_exchange(0, -1, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => {
                // 3. write message into memory
                if let Some(msg) = self.data.get_mut(writer_idx) {
                    let msg_len = core::cmp::min(data.len(), MESSAGE_SIZE);
                    msg.data[..msg_len].copy_from_slice(&data[..msg_len]);
                    msg.sender_pid = sender_pid;
                    msg.sender_tid = sender_tid;
                    let tag_len = core::cmp::min(tag.len(), MESSAGE_TAG_LEN);
                    msg.tag[..tag_len].copy_from_slice(&tag.as_bytes()[..tag_len]);
                    self.mailbox[writer_idx].store(msg_len as i16, Ordering::SeqCst);
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    pub fn receive(&'static self) -> Option<IpcMessageWrapper> {

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

        // pust Message into Wrapper to enable automatic clearing of the queue, once the value is dropped
        self.data.get(reader_idx).map(|msg| IpcMessageWrapper { msg, queue_ptr: &self.mailbox[reader_idx] })

        //Some(IpcMessageData { data: &self.data[reader_idx], msg_len: msg_len as usize, queue_ptr: ptr::from_ref(&self.mailbox[reader_idx]) })
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
        Self { writer_idx: AtomicU16::new(0), reader_idx: AtomicU16::new(0), mailbox: [const { AtomicI16::new(0) }; _], data: [IpcMessage::new(); MAX_NUM_MESSAGES] }
    }
}

impl Default for IpcQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// type used for receiving messages, and dropping them after use
pub struct IpcMessageWrapper {
    pub msg: &'static IpcMessage,
    queue_ptr: &'static AtomicI16,
}

impl Drop for IpcMessageWrapper {
    fn drop(&mut self) {
        self.queue_ptr.store(0, Ordering::SeqCst);
    }
}

impl Deref for IpcMessageWrapper {
    type Target = IpcMessage;

    fn deref(&self) -> &Self::Target {
        self.msg
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IpcMessage {
    data: [u8; MESSAGE_SIZE],
    sender_pid: Uuid,
    sender_tid: usize,
    tag: [u8; MESSAGE_TAG_LEN],
}

impl IpcMessage {
    pub fn new() -> Self {
        Self { data: [0; MESSAGE_SIZE], sender_pid: Uuid::nil(), sender_tid: 0, tag: [0; MESSAGE_TAG_LEN] }
    }
    pub fn data(&self) -> &[u8; MESSAGE_SIZE] {
        &self.data
    }
}

impl Default for IpcMessage {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for IpcMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "(pid: {:?}, tid: {}, {}): {:?}", self.sender_pid, self.sender_tid, core::str::from_utf8(&self.tag).unwrap_or("ERR"), self.data)
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

pub fn send(pid: Uuid, tid: usize, tag: &str, message: &[u8]) -> SyscallResult {

    let params = IpcSyscallParams {
        pid,
        tid,
        data_ptr: message.as_ptr(),
        data_len: message.len(),
        tag_ptr: tag.as_ptr(),
        tag_len: tag.len(),
    };

    syscall(SystemCall::IpcSend, &[core::ptr::from_ref(&params) as usize])
}

pub fn receive() -> Option<IpcMessageWrapper> {
    let ipc_queue = unsafe { (USER_SPACE_MAILBOX_START as *const IpcQueue).as_ref() };
    ipc_queue.and_then(|iq| iq.receive())
}

pub fn receive_blocking(timeout_ms: usize) -> Result<IpcMessageWrapper, syscall::return_vals::Errno> {
    match syscall(SystemCall::IpcReceiveBlocking, &[timeout_ms]) {
        Ok(_) => receive().ok_or(syscall::return_vals::Errno::EEXIST),
        Err(e) => Err(e),
    }
}