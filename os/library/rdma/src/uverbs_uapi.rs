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

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct UserMemory {
    pub in_address: u64,
    pub out_address: u64,
    pub in_size: u32,
    pub out_size: u32,
}

impl UserMemory {
    pub fn with_in_from_ref<T>(mut self, in_ref: &T) -> Self {
        self.in_address = in_ref as *const T as u64;
        self.in_size = size_of::<T>() as u32;
        self
    }

    pub fn with_in_from_slice<T>(mut self, in_slice: &[T]) -> Self {
        self.in_address = in_slice.as_ptr() as u64;
        self.in_size = (in_slice.len() * size_of::<T>()) as u32;
        self
    }

    pub fn with_out_from_ref<T>(mut self, out_ref: &mut T) -> Self {
        self.out_address = out_ref as *mut T as u64;
        self.out_size = size_of::<T>() as u32;
        self
    }

    pub fn with_out_from_slice<T>(mut self, out_slice: &mut [T]) -> Self {
        self.out_address = out_slice.as_mut_ptr() as u64;
        self.out_size = (out_slice.len() * size_of::<T>()) as u32;
        self
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
    pub index: u32,
    pub addr: usize,
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

impl From<(u32, usize, u32, u32)> for CreateMrResponse {
    fn from(value: (u32, usize, u32, u32)) -> Self {
        CreateMrResponse { index: value.0, addr: value.1, lkey: value.2, rkey: value.3 }
    }
}