//! This module contains some structs for InfiniBand.

extern crate alloc;

use bitflags::bitflags;
use strum_macros::FromRepr;

#[cfg(feature = "serialize")]
use bincode::{Decode, Encode, BorrowDecode};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Debug)]
#[non_exhaustive]
#[repr(u8)]
pub enum QueuePairType {
    RC, UC, UD,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct QueuePairCapabilities {
    pub max_send_wr: u32,
    pub max_recv_wr: u32,
    pub max_send_sge: u32,
    pub max_recv_sge: u32,
    pub max_inline_data: u32,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct DeviceHandle(usize);

impl From<usize> for DeviceHandle {
    fn from(value: usize) -> Self {
        DeviceHandle(value)
    }
}

impl Into<usize> for DeviceHandle {
    fn into(self) -> usize {
        self.0
    }
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct ProtectionDomainHandle(pub u32);

bitflags! {
    #[derive(Default, Clone, Copy)]
    pub struct AccessFlags: i32 {
        const LOCAL_WRITE = 1;
        const REMOTE_WRITE = 2;
        const REMOTE_READ = 4;
        const REMOTE_ATOMIC = 8;
        const MW_BIND = 16;
        const ZERO_BASED = 32;
        const ON_DEMAND = 64;
        const HUGETLB = 128;
        const RELAXED_ORDERING = 1048576;
    }
}

#[derive(Default, Clone, Copy)]
pub struct DeviceAttr {
    pub fw_ver_major: u16,
    pub fw_ver_minor: u16,
    pub fw_ver_subminor: u16,
    pub phys_port_cnt: u8,
}

#[repr(u8)]
#[derive(Default, Debug, Clone, Copy, FromRepr)]
pub enum Mtu {
    Mtu256 = 1,
    Mtu512 = 2,
    Mtu1024 = 3,
    Mtu2048 = 4,
    #[default]
    Mtu4096 = 5,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, FromRepr)]
#[repr(i32)]
pub enum PortState {
    #[default]
    Nop = 0,
    Down = 1,
    Init = 2,
    Armed = 3,
    Active = 4,
    ActiveDefer = 5,
}

#[derive(Debug, Default, Copy, Clone, FromRepr)]
#[repr(u8)]
pub enum PhysicalPortState {
    #[default]
    Nop = 0,
    Sleep = 1,
    Polling = 2,
    Disabled = 3,
    PortConfigurationTraining = 4,
    LinkUp = 5,
    LinkErrorRecovery = 6,
    PhyTest = 7,
}

/// A Global identifier for ibv.
#[cfg_attr(feature = "serialize", derive(Encode, Decode))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Default, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Gid {
    pub raw: [u8; 16],
}

impl Gid {
    #[allow(dead_code)]
    fn subnet_prefix(&self) -> u64 {
        u64::from_be_bytes(self.raw[..8].try_into().unwrap())
    }

    #[allow(dead_code)]
    fn interface_id(&self) -> u64 {
        u64::from_be_bytes(self.raw[8..].try_into().unwrap())
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct GlobalRoute {
    pub dgid: Gid,
    pub hop_limit: u8,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct AddressHandleAttr {
    pub grh: GlobalRoute,
    pub dlid: u16,
    pub sl: u8,
    pub src_path_bits: u8,
    pub is_global: u8,
    pub port_num: u8,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct QueuePairAttr {
    pub qp_state: QueuePairtState,
    pub path_mtu: Mtu,
    pub qkey: u32,
    pub rq_psn: u32,
    pub sq_psn: u32,
    pub dest_qp_num: u32,
    pub qp_access_flags: AccessFlags,
    pub ah_attr: AddressHandleAttr,
    pub alt_ah_attr: AddressHandleAttr,
    pub pkey_index: u16,
    pub alt_pkey_index: u16,
    pub max_rd_atomic: u8,
    pub max_dest_rd_atomic: u8,
    pub min_rnr_timer: u8,
    pub port_num: u8,
    pub timeout: u8,
    pub retry_cnt: u8,
    pub rnr_retry: u8,
    pub alt_port_num: u8,
    pub alt_timeout: u8,
}


bitflags! {
    #[derive(Clone, Copy)]
    pub struct QueuePairAttrMask: u32 {
        const IBV_QP_STATE = 1 << 0;
        const IBV_QP_ACCESS_FLAGS = 1 << 3;
        const IBV_QP_PKEY_INDEX = 1 << 4;
        const IBV_QP_PORT = 1 << 5;
        const IBV_QP_QKEY = 1 << 6;
        const IBV_QP_AV = 1 << 7;
        const IBV_QP_PATH_MTU = 1 << 8;
        const IBV_QP_TIMEOUT = 1 << 9;
        const IBV_QP_RETRY_CNT = 1 << 10;
        const IBV_QP_RNR_RETRY = 1 << 11;
        const IBV_QP_RQ_PSN = 1 << 12;
        const IBV_QP_MAX_QP_RD_ATOMIC = 1 << 13;
        const IBV_QP_ALT_PATH = 1 << 14;
        const IBV_QP_MIN_RNR_TIMER = 1 << 15;
        const IBV_QP_SQ_PSN = 1 << 16;
        const IBV_QP_MAX_DEST_RD_ATOMIC = 1 << 17;
        const IBV_QP_DEST_QPN = 1 << 20;
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct PortAttr {
    pub state: PortState,
    pub max_mtu: Mtu,
    pub active_mtu: Mtu,
    pub port_cap_flags: u32,
    pub lid: u16,
    pub sm_lid: u16,
    pub lmc: u8,
    pub link_layer: u8,
    pub phys_state: PhysicalPortState,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum QueuePairtState {
    #[default]
    Reset,
    Init,
    ReadyToReceive,
    ReadyToSend,
    SQD,
}

#[derive(Debug, Copy, Clone, Encode, Decode)]
pub struct SendWorkRequestAddressHandle {
    pub port: u32,
    pub dlid: u16,
    pub slid: u8,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct SendFlags: u32 {
        const SIGNALED   = 1 << 0;
        const FENCE      = 1 << 1;
        const SOLICITED  = 1 << 2; // event driven approach
        const INLINE     = 1 << 3; // only for very small packages
    }
}

#[cfg(feature = "serialize")]
impl Encode for SendFlags {
    fn encode<E: bincode::enc::Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
        bincode::Encode::encode(&self.0.0, encoder)
    }
}

#[cfg(feature = "serialize")]
impl<Context> Decode<Context> for SendFlags {
    fn decode<D: bincode::de::Decoder<Context=Context>>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError> {
        SendFlags::from_bits(bincode::Decode::decode(decoder)?)
            .ok_or(bincode::error::DecodeError::Other("failed to decode ibv_send_flags"))
    }
}

#[cfg(feature = "serialize")]
impl<'de, Context> BorrowDecode<'de, Context> for SendFlags {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context=Context>>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError> {
        SendFlags::from_bits(bincode::BorrowDecode::borrow_decode(decoder)?)
            .ok_or(bincode::error::DecodeError::Other("failed to decode ibv_send_flags"))
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Encode, Decode)]
pub struct ScatterGatherEntry {
    /// Virtual address it is translated using the region (dMPT) identified by the lkey. If lkey
    /// is a reserved lkey address translation is bypassed, so addr should be a physical address
    pub addr: u64,
    pub length: u32,
    pub lkey: u32,
}
