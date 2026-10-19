use core::fmt::{Debug, Display};

use alloc::format;
use alloc::string::String;
use uuid::Uuid;

use crate::{ENDPOINT_NAME_LEN, MESSAGE_DATA_SIZE, MESSAGE_NAME_LEN};
use d3os::{array_from_slice, copy_from_slice_sizechecked};
use d3os::const_string::ConstString;

macro_rules! num_slice_from_bytes {
    ($bytes:expr, $num:ty) => {
        core::array::from_fn(|i| $bytes.get((i * size_of::<$num>())..(i * size_of::<$num>() + size_of::<$num>())).map(|b| <$num>::from_le_bytes(b.try_into().unwrap())).unwrap_or_default())
    };
}

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

impl From<IpcMessageData> for IpcMessageType {
    fn from(value: IpcMessageData) -> Self {
        IpcMessageData::msg_type(&value)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum IpcMessageData {
    Empty,
    Raw {
        data: [u8; MESSAGE_DATA_SIZE],
        size: usize,
    },
    Usize(usize),
    UsizeSlice {
        data: [usize; const { MESSAGE_DATA_SIZE / size_of::<usize>() }],
        size: usize,
    },
    Isize(isize),
    IsizeSlice {
        data: [isize; const { MESSAGE_DATA_SIZE / size_of::<isize>() }],
        size: usize,
    },
    I32(i32),
    I32Slice {
        data: [i32; const { MESSAGE_DATA_SIZE / size_of::<i32>() }],
        size: usize,
    },
    F64(f64),
    F64Slice {
        data: [f64; const { MESSAGE_DATA_SIZE / size_of::<f64>() }],
        size: usize,
    },
    String(ConstString<MESSAGE_DATA_SIZE>),
    Struct {
        bytes: [u8; MESSAGE_DATA_SIZE],
        size: usize
    },
}

macro_rules! cast_bytes_as_number {
    ($bytes:expr, $out_type:ty) => {
        <$out_type>::from_ne_bytes(*$bytes[..size_of::<$out_type>()].as_array().unwrap())
    };
}

macro_rules! cast_bytes_as_slice {
    ($bytes:expr, $out_type:ty) => {
        unsafe { core::slice::from_raw_parts($bytes.as_ptr() as *const $out_type, $bytes.len() / size_of::<$out_type>()) }
    };
}

pub enum IpcMsgCastErr {
    /// data type is empty -> cannot contain any generic data
    Empty,
    /// data isn't empty, but is to small to contain value
    ToSmall,
    /// data is unaligned for casting
    Unaligned
}

impl IpcMessageData {
    /// actual amount of data space being used
    pub fn size(&self) -> usize {
        match self {
            IpcMessageData::Empty => 0,
            IpcMessageData::Raw { data: _, size } => *size,
            IpcMessageData::Usize(_) => size_of::<usize>(),
            IpcMessageData::UsizeSlice { data: _, size } => *size,
            IpcMessageData::Isize(_) => size_of::<isize>(),
            IpcMessageData::IsizeSlice { data: _, size } => *size,
            IpcMessageData::I32(_) => size_of::<i32>(),
            IpcMessageData::I32Slice { data: _, size } => *size,
            IpcMessageData::F64(_) => size_of::<f64>(),
            IpcMessageData::F64Slice { data: _, size } => *size,
            IpcMessageData::String(const_string) => const_string.len(),
            IpcMessageData::Struct { bytes: _, size } => *size,
        }
    }
    pub fn msg_type(&self) -> IpcMessageType {
        match self {
            IpcMessageData::Empty => IpcMessageType::Empty,
            IpcMessageData::Raw { data: _, size: _ } => IpcMessageType::Raw,
            IpcMessageData::Usize(_) => IpcMessageType::Usize,
            IpcMessageData::UsizeSlice { data: _, size: _ } => IpcMessageType::UsizeSlice,
            IpcMessageData::Isize(_) => IpcMessageType::Isize,
            IpcMessageData::IsizeSlice { data: _, size: _ } => IpcMessageType::IsizeSlice,
            IpcMessageData::I32(_) => IpcMessageType::I32,
            IpcMessageData::I32Slice { data: _, size: _ } => IpcMessageType::I32Slice,
            IpcMessageData::F64(_) => IpcMessageType::F64,
            IpcMessageData::F64Slice { data: _, size: _ } => IpcMessageType::F64Slice,
            IpcMessageData::String(_) => IpcMessageType::String,
            IpcMessageData::Struct { bytes: _, size: _ } => IpcMessageType::Struct,
        }
    }
    /// cast the contained data to an generic type `T` \
    /// this function will return `Err` if the data type is `Empty`
    pub fn as_ptr<T>(&self) -> Result<*const T, IpcMsgCastErr> {
        match self {
            IpcMessageData::Empty => Err(IpcMsgCastErr::Empty),
            IpcMessageData::Raw { data, size: _ } => Ok(data.as_ptr() as *const T),
            IpcMessageData::Usize(n) => Ok(core::ptr::from_ref(n) as *const T),
            IpcMessageData::UsizeSlice { data, size: _ } => Ok(data.as_ptr() as *const T),
            IpcMessageData::Isize(n) => Ok(core::ptr::from_ref(n) as *const T),
            IpcMessageData::IsizeSlice { data, size: _ } => Ok(data.as_ptr() as *const T),
            IpcMessageData::I32(n) => Ok(core::ptr::from_ref(n) as *const T),
            IpcMessageData::I32Slice { data, size: _ } => Ok(data.as_ptr() as *const T),
            IpcMessageData::F64(n) => Ok(core::ptr::from_ref(n) as *const T),
            IpcMessageData::F64Slice { data, size: _ } => Ok(data.as_ptr() as *const T),
            IpcMessageData::String(const_string) => Ok(const_string.as_ptr() as *const T),
            IpcMessageData::Struct { bytes, size: _ } => Ok(bytes.as_ptr() as *const T),
        }
    }
    /// reference tas generic type `T`
    pub fn ref_as_type<T>(&self) -> Result<&T, IpcMsgCastErr> {
        // check if actual data is large enough to contain T
        if self.size() < size_of::<T>() {
            return Err(IpcMsgCastErr::ToSmall);
        }
        let ptr = self.as_ptr::<T>()?;
        // check if pointer is aligned
        if ptr.is_aligned() {
            unsafe { Ok(ptr.as_ref().unwrap()) }
        } else {
            Err(IpcMsgCastErr::Unaligned)
        }
    }
    /// reference as a slice of generic type `T`
    pub fn ref_as_slice<T>(&self) -> Result<&[T], IpcMsgCastErr> {
        let ptr = self.as_ptr::<T>()?;
        // check if pointer is aligned
        if ptr.is_aligned() {
            let num_elements = self.size() / size_of::<T>();
            unsafe { Ok(core::slice::from_raw_parts(ptr, num_elements)) }
        } else {
            Err(IpcMsgCastErr::Unaligned)
        }
    }
    /// deref (i.e. copy) as the generic type `T`
    pub fn deref_as_type<T: Copy>(&self) -> Result<T, IpcMsgCastErr> {
        // check if actual data is large enough to contain T
        if self.size() < size_of::<T>() {
            return Err(IpcMsgCastErr::ToSmall);
        }
        let ptr = self.as_ptr::<T>()?;
        // check if pointer is aligned
        if ptr.is_aligned() {
            unsafe { Ok(*ptr) }
        } else {
            Err(IpcMsgCastErr::Unaligned)
        }
    }
    /// returns the underlying data as an u8 slice / bytes \
    /// if the generic cast fails, this function returns an empty slice
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            IpcMessageData::Empty => &[],
            IpcMessageData::Raw { data, size } => &data[..*size],
            IpcMessageData::String(const_string) => const_string.as_bytes(),
            _ => self.ref_as_slice().unwrap_or(&[]),
        }
    }
    pub fn set_data(&mut self, new_data: &[u8]) {
        match self {
            IpcMessageData::Empty => (),
            IpcMessageData::Raw { data, size } => *size = copy_from_slice_sizechecked!(data, new_data),
            IpcMessageData::Usize(n) => *n = cast_bytes_as_number!(new_data, usize),
            IpcMessageData::UsizeSlice { data, size } => *size = copy_from_slice_sizechecked!(data, cast_bytes_as_slice!(new_data, usize)),
            IpcMessageData::Isize(n) => *n = cast_bytes_as_number!(new_data, isize),
            IpcMessageData::IsizeSlice { data, size } => *size = copy_from_slice_sizechecked!(data, cast_bytes_as_slice!(new_data, isize)),
            IpcMessageData::I32(n) => *n = cast_bytes_as_number!(new_data, i32),
            IpcMessageData::I32Slice { data, size } => *size = copy_from_slice_sizechecked!(data, cast_bytes_as_slice!(new_data, i32)),
            IpcMessageData::F64(n) => *n = cast_bytes_as_number!(new_data, f64),
            IpcMessageData::F64Slice { data, size } => *size = copy_from_slice_sizechecked!(data, cast_bytes_as_slice!(new_data, f64)),
            IpcMessageData::String(const_string) => const_string.set_from_bytes(new_data),
            IpcMessageData::Struct { bytes, size } => *size = copy_from_slice_sizechecked!(bytes, new_data),
        };
    }
}

impl Debug for IpcMessageData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => write!(f, "Empty"),
            Self::Raw { data, size } => f.debug_struct("Raw").field("data", &&data[..*size]).finish(),
            Self::Usize(n) => f.debug_tuple("Usize").field(n).finish(),
            Self::UsizeSlice { data, size } => f.debug_struct("UsizeSlice").field("data", &&data[..*size]).finish(),
            Self::Isize(n) => f.debug_tuple("Isize").field(n).finish(),
            Self::IsizeSlice { data, size } => f.debug_struct("IsizeSlice").field("data", &&data[..*size]).finish(),
            Self::I32(n) => f.debug_tuple("I32").field(n).finish(),
            Self::I32Slice { data, size } => f.debug_struct("I32Slice").field("data", &&data[..*size]).finish(),
            Self::F64(n) => f.debug_tuple("F64").field(n).finish(),
            Self::F64Slice { data, size } => f.debug_struct("F64Slice").field("data", &&data[..*size]).finish(),
            Self::String(n) => f.debug_tuple("String").field(n).finish(),
            Self::Struct { bytes, size } => f.debug_struct("Struct").field("bytes", &&bytes[..*size]).finish(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IpcMessage {
    pub(crate) name: ConstString<MESSAGE_NAME_LEN>,
    pub(crate) data: IpcMessageData,
    pub(crate) sender_pid: Uuid,
    pub(crate) sender_tid: usize,
    pub(crate) callback: ConstString<ENDPOINT_NAME_LEN>,
}

impl IpcMessage {
    #![allow(clippy::missing_safety_doc)]

    pub fn new() -> Self {
        Self { name: ConstString::new(), data: IpcMessageData::Empty, sender_pid: Uuid::nil(), sender_tid: 0, callback: ConstString::new() }
    }
    pub fn new_from(name: &str, data: IpcMessageData, sender_pid: Uuid, sender_tid: usize, callback: &str) -> Self {
        Self { name: name.into(), data, sender_pid, sender_tid, callback: callback.into() }
    }

    pub fn is_empty(&self) -> bool {
        self.data.size() == 0
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
    pub fn data(&self) -> &IpcMessageData {
        &self.data
    }
    pub fn sender_pid(&self) -> &Uuid {
        &self.sender_pid
    }
    pub fn sender_tid(&self) -> usize {
        self.sender_tid
    }
    pub fn data_as_str(&self) -> Result<&str, &IpcMessageData> {
        match self.data() {
            IpcMessageData::String(const_string) => Ok(const_string.as_str_unchecked()),
            IpcMessageData::Raw { data, size } => Ok(str::from_utf8(&data[..*size]).unwrap_or("UTF8-Error")),
            IpcMessageData::Empty => Ok(""),
            _ => Err(self.data())
        }
    }
    
    pub fn copy_from(&mut self, other_msg: &IpcMessage) {
        self.name.copy_from(other_msg.name);
        self.data = other_msg.data;
        self.sender_pid = other_msg.sender_pid;
        self.sender_tid = other_msg.sender_tid;
    }
    pub fn set(&mut self, name: &str, data: IpcMessageData, sender_pid: Uuid, sender_tid: usize) {
        self.name.set(name);
        self.data = data;
        self.sender_pid = sender_pid;
        self.sender_tid = sender_tid;
    }
    pub fn set_from_data_bytes(&mut self, name: &str, bytes: &[u8], data_type: IpcMessageType, sender_pid: Uuid, sender_tid: usize, callback: &str) {
        self.name.set(name);
        // unsafe copy_from_slice directly into the enum + change discriminant manually may be faster
        //      we wouldnt have to set unused part of array to zero
        self.data = match data_type {
            IpcMessageType::Empty => IpcMessageData::Empty,
            IpcMessageType::Raw => IpcMessageData::Raw { data: core::array::from_fn(|i| bytes.get(i).copied().unwrap_or_default()), size: bytes.len() },
            IpcMessageType::Usize => IpcMessageData::Usize(usize::from_le_bytes(bytes[..size_of::<usize>()].try_into().unwrap())),
            IpcMessageType::UsizeSlice => IpcMessageData::UsizeSlice { data: num_slice_from_bytes!(bytes, usize), size: bytes.len() / size_of::<usize>() },
            IpcMessageType::Isize => IpcMessageData::Isize(isize::from_le_bytes(bytes[..size_of::<isize>()].try_into().unwrap())),
            IpcMessageType::IsizeSlice => IpcMessageData::IsizeSlice { data: num_slice_from_bytes!(bytes, isize), size: bytes.len() / size_of::<isize>() },
            IpcMessageType::I32 => IpcMessageData::I32(i32::from_le_bytes(bytes[..size_of::<i32>()].try_into().unwrap())),
            IpcMessageType::I32Slice => IpcMessageData::I32Slice { data: num_slice_from_bytes!(bytes, i32), size: bytes.len() / size_of::<i32>() },
            IpcMessageType::F64 => IpcMessageData::F64(f64::from_le_bytes(bytes[..size_of::<f64>()].try_into().unwrap())),
            IpcMessageType::F64Slice => IpcMessageData::F64Slice { data: num_slice_from_bytes!(bytes, f64), size: bytes.len() / size_of::<f64>() },
            IpcMessageType::String => IpcMessageData::String(bytes.into()),
            IpcMessageType::Struct => IpcMessageData::Struct { bytes: core::array::from_fn(|i| bytes.get(i).copied().unwrap_or_default()), size: bytes.len() },
        };
        self.sender_pid = sender_pid;
        self.sender_tid = sender_tid;
        self.callback.set(callback);
    }
    
    pub fn data_as_bytes(&self) -> &[u8] {
        IpcMessageData::as_bytes(&self.data)
    }
}

impl Default for IpcMessage {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for IpcMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let content = match self.data() {
            IpcMessageData::String(const_string) => &format!("String: {}", const_string.as_str_unchecked()),
            _ => &format!("{:?}", self.data),
        };
        write!(f, "({}, pid: {:?}, tid: {}): {}", self.name, self.sender_pid, self.sender_tid, content)
    }
}


/// Trait for variable types which may be send as message content
/// this should effectively only be implemented for references --> the data needs to exist long enough to be copied into the receiver
pub trait IpcMessageContent {
    fn to_bytes(&self) -> &[u8];
    fn msg_type(&self) -> IpcMessageType;
    fn to_data(&self) -> IpcMessageData;
}

impl IpcMessageContent for &[u8] {
    fn to_bytes(&self) -> &[u8] {
        self
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::Raw
    }
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::Raw { data: array_from_slice!(self), size: self.len() }
    }
}

impl<const N: usize> IpcMessageContent for &[u8; N] {
    fn to_bytes(&self) -> &[u8] {
        self.as_slice()
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::Raw
    }
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::Raw { data: array_from_slice!(self), size: self.len() }
    }
}

impl IpcMessageContent for &str {
    fn to_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::String
    }
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::String(ConstString::from(*self))
    }
}

impl IpcMessageContent for &String {
    fn to_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::String
    }
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::String(ConstString::from(self.as_str()))
    }
}

impl IpcMessageContent for IpcMessageData {
    fn to_bytes(&self) -> &[u8] {
        IpcMessageData::as_bytes(self)
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageData::msg_type(self)
    }
    fn to_data(&self) -> IpcMessageData {
        *self
    }
}

impl IpcMessageContent for () {
    fn to_bytes(&self) -> &[u8] {
        &[]
    }

    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::Empty
    }

    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::Empty
    }
}


/// trait used for receiving messages, and dropping them after use, to achieve one copy behaviour
pub trait IpcMessageWrapper {
    fn msg(&self) -> &IpcMessage;
}