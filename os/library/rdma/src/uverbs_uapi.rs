use alloc::vec::Vec;
use bincode::{Decode, Encode};
use num_enum::TryFromPrimitive;

use super::ib_core::*;

#[repr(u8)]
#[derive(Debug, Copy, Clone, TryFromPrimitive)]
pub enum UverbsInnerCmd {
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

type MagicHeader  = u16;
type CommandSize  = u16;
type MinorPresent = u8;
type CommandNum   = u8;

pub enum UverbsCmd {
    Call(UverbsInnerCmd, CommandNum, CommandSize, MagicHeader, MinorPresent)
}

// Define bit widths for each field
pub const UVERBS_CMD_BITS: u64 = 8;
pub const UVERBS_NR_BITS: u64 = 8;
pub const UVERBS_SIZE_BITS: u64 = 16;
pub const UVERBS_MAGIC_BITS: u64 = 16;
pub const UVERBS_MINOR_PRESENT_BITS: u64 = 1;

// Masks
pub const UVERBS_CMD_MASK: u64 = (1 << UVERBS_CMD_BITS) - 1;
pub const UVERBS_NR_MASK: u64 = (1 << UVERBS_NR_BITS) - 1;
pub const UVERBS_SIZE_MASK: u64 = (1 << UVERBS_SIZE_BITS) - 1;
pub const UVERBS_MAGIC_MASK: u64 = (1 << UVERBS_MAGIC_BITS) - 1;
pub const UVERBS_MINOR_PRESENT_MASK: u64 = (1 << UVERBS_MINOR_PRESENT_BITS) - 1;

pub const UVERBS_SIZE_SHIFT_IN_PLACE: u64 = UVERBS_CMD_BITS + UVERBS_NR_BITS;
pub const UVERBS_SIZE_MASK_IN_PLACE: u64 = 0xFFFF << UVERBS_SIZE_SHIFT_IN_PLACE;

pub const UVERBS_MAGIC: u16 = 0xABCD;
pub const UVERBS_MINOR_NOT_PRESENT: u8 = 0;
pub const UVERBS_MINOR_PRESENT: u8 = 1;

pub const UVERBS_MAX_USER_TRUST_SIZE: usize = 0x06400000; // allow user space to allocate up to 100MB
pub const UVERBS_MAX_USER_WC_REQ: usize = 16000;
pub const UVERBS_MAX_QUERY_DEVICES_REQ: usize = 10;

const CHAR_BUF: &[u8] = &[0u8; 64];

pub const UVERBS_CMD_QUERY_DEVICES: usize = UverbsCmd::Call(UverbsInnerCmd::QueryDevices, 1, 0, UVERBS_MAGIC, UVERBS_MINOR_NOT_PRESENT).encode();
pub const UVERBS_CMD_QUERY_DEVICE: usize = UverbsCmd::Call(UverbsInnerCmd::QueryDevice, 2, size_of::<ibv_device_attr>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_QUERY_PORT: usize = UverbsCmd::Call(UverbsInnerCmd::QueryPort, 3, size_of::<QueryPortRequest>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_REGISTER_MR: usize = UverbsCmd::Call(UverbsInnerCmd::RegMr, 4, size_of::<CreateMrRequest>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_SET_MR_SIZE: usize = UverbsCmd::Call(UverbsInnerCmd::SetMrSize, 5, size_of::<usize>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_CREATE_CQ: usize = UverbsCmd::Call(UverbsInnerCmd::CreateCq, 6, size_of::<CreateCqRequest>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_CREATE_QP: usize = UverbsCmd::Call(UverbsInnerCmd::CreateQp, 7, size_of::<CreateQpRequest>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_MODIFY_QP: usize = UverbsCmd::Call(UverbsInnerCmd::ModifyQp, 8, size_of::<ModifyQpRequest>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_POLL_CQ: usize = UverbsCmd::Call(UverbsInnerCmd::PollCq, 9, size_of::<PollCqRequest>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_POST_SEND: usize = UverbsCmd::Call(UverbsInnerCmd::OpPostSend, 10, size_of::<PostSendRequest>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_POST_RECV: usize = UverbsCmd::Call(UverbsInnerCmd::OpPostRecv, 11, size_of::<PostReceiveRequest>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_DESTROY_CQ: usize = UverbsCmd::Call(UverbsInnerCmd::DestroyCq, 12, 0, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_DESTROY_QP: usize = UverbsCmd::Call(UverbsInnerCmd::DestroyQp, 13, 0, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_DEREGISTER_MR: usize = UverbsCmd::Call(UverbsInnerCmd::DeregMr, 14, 0, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();

#[macro_export]
macro_rules! UVERBS_CMD_SIZE {
    ($cmd:expr) => {
        (($cmd & $crate::uverbs_uapi::UVERBS_SIZE_MASK_IN_PLACE) >> $crate::uverbs_uapi::UVERBS_SIZE_SHIFT_IN_PLACE) as usize
    };
}

type UverbsCmdEnc = usize;
type UverbsCmdSupportedSize = usize;

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

impl UverbsCmd {
    /// Encode into a single u64
    pub const fn encode(&self) -> usize {
        match self {
            UverbsCmd::Call(cmd, seq, size, magic, minor) => {
                let inner_cmd = *cmd as u64 & UVERBS_CMD_MASK;
                let cmd_num = *seq as u64 & UVERBS_NR_MASK;
                let size_u64 = *size as u64 & UVERBS_SIZE_MASK;
                let magic_u64 = *magic as u64 & UVERBS_MAGIC_MASK;
                let minor_u64 = *minor as u64 & UVERBS_MINOR_PRESENT_MASK;

                ((minor_u64 << (UVERBS_SIZE_BITS + UVERBS_NR_BITS + UVERBS_CMD_BITS + UVERBS_MAGIC_BITS)) |
                (magic_u64 << (UVERBS_SIZE_BITS + UVERBS_NR_BITS + UVERBS_CMD_BITS)) |
                (size_u64 << (UVERBS_NR_BITS + UVERBS_CMD_BITS)) |
                (cmd_num << UVERBS_CMD_BITS) |
                inner_cmd) as usize
            }
        }
    }

    /// Decode from u64
    pub fn decode(encoded: u64) -> Self {
        let cmd_num = (encoded & UVERBS_CMD_MASK) as u8;
        let seq = ((encoded >> UVERBS_CMD_BITS) & UVERBS_NR_MASK) as u8;
        let size = ((encoded >> (UVERBS_CMD_BITS + UVERBS_NR_BITS)) & UVERBS_SIZE_MASK) as u16;
        let magic = ((encoded >> (UVERBS_CMD_BITS + UVERBS_NR_BITS + UVERBS_SIZE_BITS)) & UVERBS_MAGIC_MASK) as u16;
        let minor = ((encoded >> (UVERBS_CMD_BITS + UVERBS_NR_BITS + UVERBS_SIZE_BITS + UVERBS_MAGIC_BITS)) & UVERBS_MINOR_PRESENT_MASK) as u8;

        UverbsCmd::Call(UverbsInnerCmd::try_from(cmd_num).unwrap(), seq, size, magic, minor)
    }

    /// Accessors
    pub fn cmd(&self) -> UverbsInnerCmd {
        match self {
            UverbsCmd::Call(cmd, _, _, _, _) => *cmd,
        }
    }

    pub fn seq(&self) -> u8 {
        match self {
            UverbsCmd::Call(_, seq, _, _, _) => *seq,
        }
    }

    pub fn size(&self) -> u16 {
        match self {
            UverbsCmd::Call(_, _, size, _, _) => *size,
        }
    }

    pub fn magic(&self) -> u16 {
        match self {
            UverbsCmd::Call(_, _, _, magic, _) => *magic,
        }
    }

    pub fn decompose(&self) -> (UverbsInnerCmd, u8, u16, u16, u8) {
        match self {
            UverbsCmd::Call(cmd, seq, size, magic, minor) => (*cmd, *seq, *size, *magic, *minor),
        }
    }
}

/// Accessors work from the encoded value
pub fn cmd(encoded: u64) -> UverbsInnerCmd {
    let cmd_num = (encoded & UVERBS_CMD_MASK) as u8;
    UverbsInnerCmd::try_from(cmd_num).unwrap()
}

pub fn seq(encoded: u64) -> u8 {
    ((encoded >> UVERBS_CMD_BITS) & UVERBS_NR_MASK) as u8
}

pub fn size(encoded: u64) -> u32 {
    ((encoded >> (UVERBS_NR_BITS + UVERBS_CMD_BITS)) & UVERBS_SIZE_MASK) as u32
}

pub fn magic(encoded: u64) -> u16 {
    ((encoded >> (UVERBS_SIZE_BITS + UVERBS_NR_BITS + UVERBS_CMD_BITS)) & UVERBS_MAGIC_MASK) as u16
}

pub fn minor_present(encoded: u64) -> u8 {
    ((encoded >> (UVERBS_SIZE_BITS + UVERBS_NR_BITS + UVERBS_CMD_BITS + UVERBS_MINOR_PRESENT_BITS)) & UVERBS_MINOR_PRESENT_MASK) as u8
}