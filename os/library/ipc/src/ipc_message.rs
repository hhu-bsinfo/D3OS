use core::fmt::Display;
use core::sync::atomic::{AtomicI16, Ordering};
use core::ops::Deref;

use alloc::format;
use alloc::string::String;
use uuid::Uuid;

use crate::{MESSAGE_DATA_SIZE, MESSAGE_NAME_LEN, MESSAGE_TAG_LEN};
use crate::const_string::ConstString;


/// type used for receiving messages, and dropping them after use
pub struct IpcMessageWrapper {
    pub msg: &'static IpcMessage,
    pub(crate) queue_ptr: &'static AtomicI16,
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
    pub(crate) name: ConstString<MESSAGE_NAME_LEN>,
    pub(crate) msg_type: ConstString<MESSAGE_TAG_LEN>,
    pub(crate) data: [u8; MESSAGE_DATA_SIZE],
    pub(crate) data_len: usize,
    pub(crate) sender_pid: Uuid,
    pub(crate) sender_tid: usize,
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

impl<const N: usize> IpcMessageContent for &[u8; N] {
    fn to_bytes(&self) -> &[u8] {
        self.as_slice()
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

impl<const N: usize> IpcMessageContent for &[usize; N] {
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