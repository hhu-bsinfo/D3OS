use core::fmt::{Debug, Display};
use core::ops::{Deref, Index, IndexMut};
use core::slice::SliceIndex;
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
    pub msg_name_ptr: *const u8,
    pub msg_name_len: usize,
    pub msg_type_ptr: *const u8,
    pub msg_type_len: usize,
    pub data_ptr: *const u8,
    pub data_len: usize,
}

const MESSAGE_NAME_LEN: usize = 0x20;
const MESSAGE_TAG_LEN: usize = 0x10;
//pub const MESSAGE_DATA_SIZE: usize = 0x100 - (size_of::<Uuid>() + size_of::<usize>() + MESSAGE_TAG_LEN);
pub const MESSAGE_DATA_SIZE: usize = 0x100;
const MAX_NUM_MESSAGES: usize = 32; // !this needs to be a power of 2! also this can not be smaller than 4

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
    name: ConstString<MESSAGE_NAME_LEN>,
    msg_type: ConstString<MESSAGE_TAG_LEN>,
    data: [u8; MESSAGE_DATA_SIZE],
    data_len: usize,
    sender_pid: Uuid,
    sender_tid: usize,
}

impl IpcMessage {
    pub fn new() -> Self {
        Self { name: ConstString::new(), msg_type: ConstString::new(), data: [0; _], data_len: 0, sender_pid: Uuid::nil(), sender_tid: 0 }
    }
    pub fn is_empty(&self) -> bool {
        self.data_len == 0
    }
    pub fn name_as_str(&self) -> &str {
        self.name.as_str()
    }
    pub fn msg_type_as_str(&self) -> &str {
        self.msg_type.as_str()
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

impl Default for IpcMessage {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for IpcMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let content = match self.msg_type_as_str() {
            "STRING" => self.data_as_str(),
            "USIZE" => &format!("{:?}", self.data_as_usize_slice()),
            _ => &format!("{:?}", self.data),
        };
        write!(f, "({}, {}, pid: {:?}, tid: {}): {:?}", self.name, self.msg_type_as_str(), self.sender_pid, self.sender_tid, content)
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

pub fn send<T: IpcMessageContent>(service: &str, msg_name: &str, message: T) -> SyscallResult {
    let bytes: &[u8] = message.to_bytes();
    send_internal(service, msg_name, message.tag(), bytes)
}

pub fn send_with_tag<T: IpcMessageContent>(service: &str, msg_name: &str, tag: &str, message: T) -> SyscallResult {
    let bytes = message.to_bytes();
    send_internal(service, msg_name, tag, bytes)
}

pub fn answer<T: IpcMessageContent>(original_message: &IpcMessage, message: T) -> SyscallResult {
    let bytes = message.to_bytes();
    send_internal(&format!("callback-{}", original_message.sender_pid), &format!("answer-{}", original_message.name), message.tag(), bytes)
}

pub fn answer_with_name<T: IpcMessageContent>(original_message: &IpcMessage, msg_name: &str, message: T) -> SyscallResult {
    let bytes = message.to_bytes();
    send_internal(&format!("callback-{}", original_message.sender_pid), msg_name, message.tag(), bytes)
}

pub fn send_internal(service: &str, msg_name: &str, tag: &str, message: &[u8]) -> SyscallResult {

    // register callback endpoint
    let pid = process::current().id();
    let _res = register_ipc_endpoint(&format!("callback-{}", pid));

    let params = IpcSyscallParams {
        service_name_ptr: service.as_ptr(),
        service_name_len: service.len(),
        msg_name_ptr: msg_name.as_ptr(),
        msg_name_len: msg_name.len(),
        msg_type_ptr: tag.as_ptr(),
        msg_type_len: tag.len(),
        data_ptr: message.as_ptr(),
        data_len: message.len(),
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

#[derive(Debug, Clone, Copy)]
/// nul-terminated const sized string
pub struct ConstString<const T: usize> {
    buffer: [u8; T],
    len: usize,
}

impl <const T: usize> ConstString<T> {
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buffer[..self.len]).unwrap_or("UTF8-ERROR")
    }
    pub fn len(&self) -> usize {
        self.len
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    pub fn set(&mut self, s: &str) {
        let l = core::cmp::min(T, s.len());
        self.buffer[..l].copy_from_slice(&s.as_bytes()[..l]);
        self.len = l;
    }
    pub fn clear(&mut self) {
        self.buffer.fill(0);
        self.len = 0;
    }
    pub fn append(&mut self, s: &str) {
        let idx = self.len();
        let space_left = T - idx;
        let l = core::cmp::min(space_left, s.len());
        self.buffer[..l].copy_from_slice(&s.as_bytes()[..l]);
        self.len = l;
    }
}

impl ConstString<0> {
    pub fn empty() -> Self {
        Self { buffer: [0; 0], len: 0 }
    }
}

impl <const T: usize> ConstString<T> {
    pub fn new() -> Self {
        Self { buffer: [0; T], len: 0 }
    }
}

impl<const T: usize> Default for ConstString<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl <const T: usize> From<[u8; T]> for ConstString<T> {
    fn from(value: [u8; T]) -> Self {
        Self { buffer: value, len: value.len() }
    }
}

impl <const T: usize, I: SliceIndex<[u8]>> Index<I> for ConstString<T> {
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        Index::index(&self.buffer, index)
    }
}

impl <const T: usize, I: SliceIndex<[u8]>> IndexMut<I> for ConstString<T> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut self.buffer, index)
    }
}

impl <const T: usize> Display for ConstString<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}