use alloc::vec::Vec;
use bincode::{Decode, Encode};

use super::ib_core::*;

#[repr(u64)]
#[derive(Debug, Copy, Clone)]
pub enum UverbsCmd {
    // Completion queue operations
    CreateCq       = 1,
    DestroyCq      = 2,
    PollCq         = 3,

    // Queue pair operations
    CreateQp       = 4,
    ModifyQp       = 5,
    QueryQp        = 6,
    DestroyQp      = 7,
    OpPostSend     = 8,
    OpPostRecv     = 9,

    // Memory region operations
    RegMr          = 10,
    DeregMr        = 11,
    SetMrSize      = 12,

    // Query
    QueryDevice    = 13,
    QueryPort      = 14,
    QueryDevices   = 15,
}

pub const UVERBS_MAX_USER_TRUST_SIZE: usize = 0x06400000; // allow user space to allocate up to 100MB
pub const UVERBS_MAX_USER_WC_REQ: usize = 16000;
pub const UVERBS_MAX_QUERY_DEVICES_REQ: usize = 10;

const CHAR_BUF: &[u8] = &[0u8; 64];

/// A region of user memory, described by its start address and its size in bytes.
///
/// The uverbs system call takes two of these: one for the request (in) and one
/// for the response (out) buffer. An empty slice (address and size zero) means
/// that the command does not use that direction.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct UserSlice {
    pub address: u64,
    pub size: usize,
}

impl UserSlice {
    pub const EMPTY: Self = Self { address: 0, size: 0 };

    pub fn new(address: u64, size: usize) -> Self {
        Self { address, size }
    }

    pub fn from_ref<T>(value: &T) -> Self {
        Self { address: value as *const T as u64, size: size_of::<T>() }
    }

    pub fn from_mut<T>(value: &mut T) -> Self {
        Self { address: value as *mut T as u64, size: size_of::<T>() }
    }

    pub fn from_slice<T>(slice: &[T]) -> Self {
        Self { address: slice.as_ptr() as u64, size: size_of_val(slice) }
    }

    pub fn from_mut_slice<T>(slice: &mut [T]) -> Self {
        Self { address: slice.as_mut_ptr() as u64, size: size_of_val(slice) }
    }

    pub fn is_empty(&self) -> bool {
        self.address == 0 || self.size == 0
    }

    /// How many elements of type `T` fit into this slice.
    pub fn capacity<T>(&self) -> usize {
        self.size / size_of::<T>()
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct QueryPortRequest {
    pub port_num: u8
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct CreateMrRequest {
    pub ibv_access_flags: ibv_access_flags,
    pub data_ptr: *mut u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct CreateMrResponse {
    pub handle: u32,
    pub lkey: u32,
    pub rkey: u32
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct CreateCqRequest {
    pub cq_entries: i32,
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct CreateCqResponse {
    pub cq_num: u32
}


#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct PollCqRequest {
    pub cq_num: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CreateQpRequest {
    pub qp_type: ibv_qp_type::Type,
    pub send_cq_num: u32,
    pub recv_cq_num: u32,
    pub ib_caps: ibv_qp_cap,
}

#[derive(Copy, Clone)]
pub struct CreateQpResponse {
    pub qp_num: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ModifyQpRequest {
    pub qp_num: u32,
    pub attr: ibv_qp_attr,
    pub attr_mask: ibv_qp_attr_mask
}

impl Default for ibv_send_wr {
    fn default() -> Self {
        Self { 
            wr_id: Default::default(), 
            next: Default::default(), 
            sg_list: Default::default(), 
            opcode: ibv_wr_opcode::IBV_WR_SEND,
            send_flags: ibv_send_flags::SIGNALED, 
            __bindgen_anon_1: Default::default(), 
            wr: Default::default(), 
            qp_type: Default::default(), 
            __bindgen_anon_2: Default::default() }
    }
}

impl Default for ibv_recv_wr {
    fn default() -> Self {
        Self { 
            wr_id: Default::default(), 
            next: Default::default(), 
            sg_list: Default::default(), 
        }
    }
}

#[repr(C)]
#[derive(Clone, Default, Encode, Decode)]
pub struct PostSendRequest {
    pub qp_num: u32,
    pub wrs: Vec<SendWorkRequest>,
}

#[repr(C)]
#[derive(Clone, Encode, Decode)]
pub struct SendWorkRequest {
    pub wr_id: u64,
    pub sges: Vec<ibv_sge>,
    pub opcode: ibv_wr_opcode,
    pub send_flags: ibv_send_flags,
    pub wr: ibv_send_wr_wr,
}


#[repr(C)]
#[derive(Clone, Encode, Decode)]
pub struct PostReceiveRequest {
    pub qp_num: u32,
    pub wrs: Vec<ReceiveWorkRequest>,
}

#[derive(Clone, Encode, Decode)]
pub struct ReceiveWorkRequest {
    pub wr_id: u64,
    pub sges: Vec<ibv_sge>,
}