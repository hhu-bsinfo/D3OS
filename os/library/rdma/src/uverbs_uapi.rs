#![allow(non_camel_case_types)]

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
pub const UVERBS_CMD_QUERY_DEVICE: usize = UverbsCmd::Call(UverbsInnerCmd::QueryDevice, 2, size_of::<ibv_device_attr_container>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_QUERY_PORT: usize = UverbsCmd::Call(UverbsInnerCmd::QueryPort, 3, size_of::<ibv_port_attr_container>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_REGISTER_MR: usize = UverbsCmd::Call(UverbsInnerCmd::RegMr, 4, size_of::<ibv_mr_container>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_SET_MR_SIZE: usize = UverbsCmd::Call(UverbsInnerCmd::SetMrSize, 5, size_of::<usize>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_CREATE_CQ: usize = UverbsCmd::Call(UverbsInnerCmd::CreateCq, 6, size_of::<ibv_cq_container>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_CREATE_QP: usize = UverbsCmd::Call(UverbsInnerCmd::CreateQp, 7, size_of::<ibv_qp_container>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_MODIFY_QP: usize = UverbsCmd::Call(UverbsInnerCmd::ModifyQp, 8, size_of::<ibv_qp_modify_container>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_POLL_CQ: usize = UverbsCmd::Call(UverbsInnerCmd::PollCq, 9, size_of::<ibv_cq_poll_container>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_POST_SEND: usize = UverbsCmd::Call(UverbsInnerCmd::OpPostSend, 10, size_of::<ibv_qp_post_send_container>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
pub const UVERBS_CMD_POST_RECV: usize = UverbsCmd::Call(UverbsInnerCmd::OpPostRecv, 11, size_of::<ibv_qp_post_recv_container>() as u16, UVERBS_MAGIC, UVERBS_MINOR_PRESENT).encode();
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

pub trait TypeSize {
    const S: usize;
}

impl TypeSize for ibv_device_attr {
    const S: usize = CHAR_BUF.len();
}

impl TypeSize for ibv_port_attr_container {
    const S: usize = size_of::<u8>();
}

impl TypeSize for ibv_mr_container {
    const S: usize = UVERBS_MAX_USER_TRUST_SIZE;
}

impl TypeSize for ibv_qp_container {
    const S: usize = size_of::<ibv_qp_cap>();
}

impl TypeSize for ibv_qp_modify_container {
    const S: usize = size_of::<ibv_qp_attr>();
}

impl TypeSize for ibv_cq_poll_container {
    const S: usize = size_of::<ibv_wc>() * UVERBS_MAX_USER_WC_REQ;
}

impl TypeSize for ibv_qp_post_send_container {
    const S: usize = size_of::<ibv_send_wr>();
}

impl TypeSize for ibv_qp_post_recv_container {
    const S: usize = size_of::<ibv_recv_wr>();
}

// global table to define a pl. interface
static UVERBS_PER_CMD_SIZE_TABLE: &[(UverbsCmdEnc, UverbsCmdSupportedSize)] = &[
    (UVERBS_CMD_QUERY_DEVICES, UVERBS_MAX_QUERY_DEVICES_REQ * size_of::<usize>())
];

pub fn uverbs_per_cmd_size(cmd_enc: UverbsCmdEnc) -> usize {
    UVERBS_PER_CMD_SIZE_TABLE.iter()
        .find(|x| x.0 == cmd_enc)
        .map(|x| x.1).unwrap_or(0)
    }

#[repr(C)]
pub struct ibv_device_attr_container {
    pub fw_ver: [u8; ibv_device_attr::S],
    pub phys_port_cnt: u8
}

#[repr(C)]
pub struct ibv_port_attr_container {
    pub ibv_port_attr: ibv_port_attr,
    pub port_num: u8
}

#[repr(C)]
#[derive(Default)]
pub struct ibv_mr_container {
    pub ibv_access_flags: ibv_access_flags,
    pub data_ptr: *mut u8,
    pub len: usize,
    pub ibv_mr_res: ibv_mr_res
}

#[repr(C)]
#[derive(Default)]
pub struct ibv_mr_res {
    pub index: u32,
    pub addr: usize,
    pub lkey: u32,
    pub rkey: u32
}

#[repr(C)]
#[derive(Default)]
pub struct ibv_cq_container {
    pub cq_entries: i32,
    pub cq_num: u32
}

#[repr(C)]
#[derive(Default)]
pub struct ibv_cq_poll_container {
    pub wc: *mut ibv_wc,
    pub wc_len: usize,
    pub cq_num: u32,
}

#[repr(C)]
pub struct ibv_qp_container {
    pub qp_type: ibv_qp_type::Type,
    pub send_cq_num: u32,
    pub recv_cq_num: u32,
    pub ib_caps: *mut ibv_qp_cap,
    pub qp_num: u32
}

impl Default for ibv_qp_container {
    fn default() -> Self {
        Self { 
            qp_type: ibv_qp_type::IBV_QPT_RC, // just place holder
            send_cq_num: Default::default(), 
            recv_cq_num: Default::default(), 
            ib_caps: Default::default(), 
            qp_num: Default::default() 
        }
    }
}

#[repr(C)]
pub struct ibv_qp_modify_container {
    pub qp_num: u32,
    pub attr: *const ibv_qp_attr,
    pub attr_mask: ibv_qp_attr_mask
}

impl Default for ibv_qp_modify_container {
    fn default() -> Self {
        Self { 
            qp_num: Default::default(), 
            attr: Default::default(), 
            attr_mask: ibv_qp_attr_mask::IBV_QP_PORT // just place holder
        }
    }
}

impl Default for ibv_send_wr {
    fn default() -> Self {
        Self { 
            wr_id: Default::default(), 
            next: Default::default(), 
            sg_list: Default::default(), 
            num_sge: Default::default(), 
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
            num_sge: Default::default() 
        }
    }
}

#[repr(C)]
#[derive(Default)]
pub struct ibv_qp_post_send_container {
    pub ibv_send_wr: *mut ibv_send_wr,
    pub qp_num: u32
}

#[repr(C)]
#[derive(Default)]
pub struct ibv_qp_post_recv_container {
    pub ibv_recv_wr: *mut ibv_recv_wr,
    pub qp_num: u32
}

impl From<(u32, usize, u32, u32)> for ibv_mr_res {
    fn from(value: (u32, usize, u32, u32)) -> Self {
        ibv_mr_res { index: value.0, addr: value.1, lkey: value.2, rkey: value.3 }
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