use core::fmt::{Debug, Display};
use core::ops::Deref;
use core::sync::atomic::{AtomicI16, AtomicU16, Ordering};
use alloc::format;
use alloc::string::String;
use syscall::{SystemCall, return_vals::SyscallResult, syscall};
use uuid::Uuid;

use crate::process;

// Schnittstelle für Interprozesskommunikation

pub struct IpcSyscallParams {
    pub service_name_ptr: *const u8,
    pub service_name_len: usize,
    pub data_ptr: *const u8,
    pub data_len: usize,
    pub tag_ptr: *const u8,
    pub tag_len: usize,
}

const MESSAGE_TAG_LEN: usize = 0x10;
//pub const MESSAGE_DATA_SIZE: usize = 0x100 - (size_of::<Uuid>() + size_of::<usize>() + MESSAGE_TAG_LEN);
pub const MESSAGE_DATA_SIZE: usize = 0x100;
const MAX_NUM_MESSAGES: usize = 32; // !this needs to be a power of 2! also this can not be smaller than 4

#[derive(Debug)]
pub struct IpcQueue {
    writer_idx: AtomicU16,
    reader_idx: AtomicU16,
    mailbox: [AtomicI16; MAX_NUM_MESSAGES],
    data: [IpcMessage; MAX_NUM_MESSAGES],
}

impl IpcQueue {
    
    pub fn send(&self, tag: &str, data: &[u8], sender_pid: &Uuid) -> bool {

        // 1. increment writer idx
        let writer_idx = self.writer_idx().fetch_add(1, Ordering::SeqCst) as usize % MAX_NUM_MESSAGES;

        // 2. check (and lock) mailbox entry
        if self.mailbox[writer_idx].compare_exchange(0, -1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            // 3. get msg as mut
            //    -> this is sound because we ensure we are the sole thread accessing this message, using the atomic variable above
            if let Some(msg) = self.data.get(writer_idx).and_then(|msg| unsafe { core::ptr::from_ref(msg).cast_mut().as_mut() }) {
                // 4. write message
                let msg_len = core::cmp::min(data.len(), MESSAGE_DATA_SIZE);
                msg.data[..msg_len].copy_from_slice(&data[..msg_len]);
                msg.data_len = msg_len;
                msg.sender_pid = *sender_pid;
                let tag_len = core::cmp::min(tag.len(), MESSAGE_TAG_LEN);
                msg.tag[..tag_len].copy_from_slice(&tag.as_bytes()[..tag_len]);
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
            self.data.get(reader_idx).map(|msg| IpcMessageWrapper { msg, queue_ptr: &self.mailbox[reader_idx] })
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
        Self { writer_idx: AtomicU16::new(0), reader_idx: AtomicU16::new(0), mailbox: [const { AtomicI16::new(0) }; MAX_NUM_MESSAGES], data: [IpcMessage::new(); MAX_NUM_MESSAGES] }
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
    data: [u8; MESSAGE_DATA_SIZE],
    data_len: usize,
    sender_pid: Uuid,
    sender_tid: usize,
    tag: [u8; MESSAGE_TAG_LEN]
}

impl IpcMessage {
    pub fn new() -> Self {
        Self { data: [0; _], data_len: 0, sender_pid: Uuid::nil(), sender_tid: 0, tag: [0; _] }
    }
    pub fn is_empty(&self) -> bool {
        self.data_len == 0
    }
    pub fn tag(&self) -> &str {
        str_from_ut8_nul_terminated(&self.tag)
    }
    pub fn data(&self) -> &[u8] {
        &self.data[..self.data_len]
    }
    pub fn data_as_usize(&self) -> &usize {
        let ptr = self.data().as_ptr() as *const usize;
        unsafe { ptr.as_ref().unwrap() }
    }
    pub fn data_as_usize_slice(&self) -> &[usize] {
        let ptr = self.data().as_ptr() as *const usize;
        let l = self.data.len() / size_of::<usize>();
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    pub fn data_as_str(&self) -> &str {
        str::from_utf8(self.data()).unwrap_or("UTF8_ERROR")
        //str_from_ut8_nul_terminated(&self.data)
    }
}

fn str_from_ut8_nul_terminated(bytes: &[u8]) -> &str {
    let first_nul = bytes.iter().position(|c| *c == b'\0').unwrap_or(bytes.len());
    match core::str::from_utf8(bytes) {
        Ok(tag_str) => &tag_str[..first_nul],
        Err(_) => "UTF8_ERROR",
    }
}

impl Default for IpcMessage {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for IpcMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let content = match self.tag() {
            "STRING" => self.data_as_str(),
            "USIZE" => &format!("{:?}", self.data_as_usize_slice()),
            _ => &format!("{:?}", self.data),
        };
        write!(f, "(pid: {:?}, tid: {}, {}): {:?}", self.sender_pid, self.sender_tid, self.tag(), content)
    }
}

/// Trait for variable types which may be send as message content
/// this should effectively only be implemented for references --> the data needs to exist long enough to be copied into the receiver
pub trait IpcMessageContent {
    fn to_bytes(&self) -> &[u8];
    fn tag(&self) -> &str;
}

impl IpcMessageContent for &[u8] {
    fn to_bytes(&self) -> &[u8] {
        self
    }
    fn tag(&self) -> &str {
        "SIMPLE"
    }
}

impl IpcMessageContent for &[usize] {
    fn to_bytes(&self) -> &[u8] {
        let ptr = self.as_ptr() as *const u8;
        let l = core::mem::size_of_val(*self);
        // alignment should be safe, because usize alignment should always include u8 alignment
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn tag(&self) -> &str {
        "USIZE"
    }
}

impl IpcMessageContent for usize {
    fn to_bytes(&self) -> &[u8] {
        // cast an usize into an u8 array
        let ptr = core::ptr::from_ref(self) as *const u8;
        let l = size_of::<usize>();
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn tag(&self) -> &str {
        "USIZE"
    }
}

impl IpcMessageContent for &str {
    fn to_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
    fn tag(&self) -> &str {
        "STRING"
    }
}

impl IpcMessageContent for &String {
    fn to_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
    fn tag(&self) -> &str {
        "STRING"
    }
}

const USER_SPACE_MAILBOX_START: usize = 0x10100000000;

pub fn register_ipc_endpoint(svc_name: &str) -> Result<usize, syscall::return_vals::Errno> {
    // TODO what happens when we register multiple endpoints / multiple services for this endpoint
    let mailbox_start = USER_SPACE_MAILBOX_START as *const u8;
    let ipc_queue_ptr = mailbox_start as *mut IpcQueue;
    unsafe { 
        ipc_queue_ptr.write(IpcQueue::new());
    }
    let name_ptr = svc_name.as_ptr();
    let name_len = svc_name.len();
    syscall(SystemCall::IpcRegister, &[name_ptr as usize, name_len])
}

pub fn send<T: IpcMessageContent>(service: &str, message: T) -> SyscallResult {
    let bytes: &[u8] = message.to_bytes();
    send_internal(service, message.tag(), bytes)
}

pub fn send_with_tag<T: IpcMessageContent>(service: &str, tag: &str, message: T) -> SyscallResult {
    let bytes = message.to_bytes();
    send_internal(service, tag, bytes)
}

pub fn answer<T: IpcMessageContent>(original_message: &IpcMessage, message: T) -> SyscallResult {
    let bytes = message.to_bytes();
    send_internal(&format!("callback-{}", original_message.sender_pid), message.tag(), bytes)
}

pub fn send_internal(service: &str, tag: &str, message: &[u8]) -> SyscallResult {

    // register callback endpoint
    let pid = process::current().id();
    let _res = register_ipc_endpoint(&format!("callback-{}", pid));

    let params = IpcSyscallParams {
        service_name_ptr: service.as_ptr(),
        service_name_len: service.len(),
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