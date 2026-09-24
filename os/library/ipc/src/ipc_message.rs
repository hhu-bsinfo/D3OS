use core::fmt::Display;

use alloc::format;
use alloc::string::String;
use uuid::Uuid;

use crate::{MESSAGE_DATA_SIZE, MESSAGE_NAME_LEN};
use crate::const_string::{ConstString, CopySizeChecked};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcMessageType {
    Empty,
    Raw,
    Usize,
    UsizeSlice,
    Isize,
    IsizeSlice,
    I32,
    I32Slice,
    F64,
    F64Slice,
    String,
    Struct,
}

#[derive(Debug, Clone, Copy)]
pub struct IpcMessage {
    pub(crate) name: ConstString<MESSAGE_NAME_LEN>,
    pub(crate) msg_type: IpcMessageType,
    pub(crate) data: [u8; MESSAGE_DATA_SIZE],
    pub(crate) data_len: usize,
    pub(crate) sender_pid: Uuid,
    pub(crate) sender_tid: usize,
}

impl IpcMessage {
    #![allow(clippy::missing_safety_doc)]

    pub fn new() -> Self {
        Self { name: ConstString::new(), msg_type: IpcMessageType::Raw, data: [0; _], data_len: 0, sender_pid: Uuid::nil(), sender_tid: 0 }
    }
    pub fn new_from(name: &str, msg_type: IpcMessageType, data: &[u8], sender_pid: Uuid, sender_tid: usize) -> Self {
        let mut data_arr = [0; _];
        let l = data_arr.copy_from_slice_sizechecked(data);
        Self { name: name.into(), msg_type, data: data_arr, data_len: l, sender_pid, sender_tid }
    }

    pub fn is_empty(&self) -> bool {
        self.data_len == 0
    }
    pub fn is_valid(&self) -> bool {
        self.sender_tid != 0 && !self.sender_pid.is_nil()
    }
    pub fn name_as_str(&self) -> &str {
        self.name.as_str_unchecked()
    }
    pub fn name(&self) -> &ConstString<MESSAGE_NAME_LEN> {
        &self.name
    }
    pub fn msg_type(&self) -> IpcMessageType {
        self.msg_type
    }
    pub fn data(&self) -> &[u8] {
        &self.data[..self.data_len]
    }

    /// reference the data as generic type `T`
    pub unsafe fn ref_data<T>(&self) -> &T {
        let ptr = self.data().as_ptr() as *const T;
        unsafe { ptr.as_ref().unwrap() }
    }
    /// reference the data as a slice of generic type `T`
    pub unsafe fn ref_data_as_slice<T>(&self) -> &[T] {
        let ptr = self.data().as_ptr() as *const T;
        let l = self.data_len / size_of::<T>();
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    /// deref (i.e. copy) the data as the generic type `T`
    pub unsafe fn deref_data<T: Copy>(&self) -> T {
        let ptr = self.data().as_ptr() as *const T;
        unsafe { *ptr.as_ref().unwrap() }
    }

    pub fn data_as_str(&self) -> Result<&str, IpcMessageType> {
        if self.msg_type == IpcMessageType::String {
            str::from_utf8(self.data()).or(Err(self.msg_type))
        } else {
            Err(self.msg_type)
        }
    }
    pub fn data_as_str_unchecked(&self) -> &str {
        str::from_utf8(self.data()).unwrap_or("UTF8_ERROR")
    }
    pub fn sender_pid(&self) -> &Uuid {
        &self.sender_pid
    }
    pub fn sender_tid(&self) -> usize {
        self.sender_tid
    }

    pub fn copy_from(&mut self, other_msg: &IpcMessage) {
        self.name.copy_from(other_msg.name);
        self.msg_type = other_msg.msg_type;
        let l = other_msg.data_len;
        self.data[..l].copy_from_slice(other_msg.data());
        self.data_len = l;
        self.sender_pid = other_msg.sender_pid;
        self.sender_tid = other_msg.sender_tid;
    }
    pub fn set(&mut self, name: &str, msg_type: IpcMessageType, data: &[u8], sender_pid: Uuid, sender_tid: usize) {
        self.name.set(name);
        self.msg_type = msg_type;
        let l = self.data.copy_from_slice_sizechecked(data);
        self.data_len = l;
        self.sender_pid = sender_pid;
        self.sender_tid = sender_tid;
    }
}

impl Default for IpcMessage {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for IpcMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let content = match self.msg_type {
            IpcMessageType::String => self.data_as_str_unchecked(),
            IpcMessageType::Usize => &format!("{:?}", self.data_as_usize()),
            IpcMessageType::UsizeSlice => &format!("{:?}", self.data_as_usize_slice()),
            _ => &format!("{:?}", self.data),
        };
        write!(f, "({}, {:?}, pid: {:?}, tid: {}): {:?}", self.name, self.msg_type, self.sender_pid, self.sender_tid, content)
    }
}


/// Trait for variable types which may be send as message content
/// this should effectively only be implemented for references --> the data needs to exist long enough to be copied into the receiver
pub trait IpcMessageContent {
    fn to_bytes(&self) -> &[u8];
    fn msg_type(&self) -> IpcMessageType;
}

impl IpcMessageContent for &[u8] {
    fn to_bytes(&self) -> &[u8] {
        self
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::Raw
    }
}

impl<const N: usize> IpcMessageContent for &[u8; N] {
    fn to_bytes(&self) -> &[u8] {
        self.as_slice()
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::Raw
    }
}

impl IpcMessageContent for &str {
    fn to_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::String
    }
}

impl IpcMessageContent for &String {
    fn to_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::String
    }
}


/// trait used for receiving messages, and dropping them after use, to achieve one copy behaviour
pub trait IpcMessageWrapper {
    fn msg(&self) -> &IpcMessage;
}