use strum_macros::FromRepr;
use super::ib_core::*;

#[repr(u64)]
#[derive(Debug, Copy, Clone, FromRepr)]
pub enum UverbsCmd {
    // Device operations
    QueryDevice = 1,
    QueryPort,
    QueryDevices,

    // Protection Domain operations
    AllocPd,
    DeallocPd,

    // Completion queue operations
    CreateCq,
    DestroyCq,

    // Queue pair operations
    CreateQp,
    ModifyQp,
    QueryQp,
    DestroyQp,

    // Memory region operations
    RegMr,
    DeregMr,
    SetMrSize,

    /// Drain the device's event queue (port-down/QP-error/internal-error notifications). Since
    /// posting and polling no longer go through the kernel on every operation, userspace calls
    /// this itself, rate-limited, from its poll loop instead of relying on it piggybacking on
    /// another verb.
    DrainEvents,

    // Fallback data-path operations, if they are not supported by the user-space driver
    //PollCq,
    //PostSend,
    //PostRecv,

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
pub struct AllocPdResponse {
    pub pd: u32,
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct DeallocPdRequest {
    pub pd: u32,
}


#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct CreateMrRequest {
    pub pd: u32,
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
#[derive(Copy, Clone)]
pub struct CreateCqRequest {
    pub cq_entries: i32,

    // mlx4 specific, under linux this is an opaque driver_data[]: the userspace-owned, -mmap'd
    // CQE ring and its consumer-index/arm-index doorbell record. The kernel only builds an MTT
    // over `buffer` and runs the CMD-interface transition; polling and CQE parsing happen
    // entirely in userspace against these from here on.
    pub buffer: *const u8,
    /// CQ doorbell records are aligned on an 8 B boundary per the PRM.
    pub doorbell_ptr: *const u64,
}

#[derive(Copy, Clone)]
pub struct CreateCqResponse {
    pub cq_num: u32,
    /// The UAR page mapped into the calling process, for ringing the arm doorbell.
    pub doorbell_page: *mut u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CreateQpRequest {
    pub pd: u32,
    pub send_cq_num: u32,
    pub recv_cq_num: u32,
    pub qp_type: ibv_qp_type::Type,
    pub _sq_sig_all: u8,
    pub _reserved: u16,

    // mlx4 specific, under linux this is an opaque driver_data[]
    pub buffer: *const u8,
    pub doorbell_ptr: *const u32,
    pub log_sq_bb_count: u8,
    pub log_sq_stride: u8,
    pub inline_recv_size: u16,

    // these fields are not part of the linux uverbs struct, but are calculated based on the capabilities
    pub log_rq_wqe_count: u8,
    pub log_rq_stride: u8,
}

#[derive(Copy, Clone)]
pub struct CreateQpResponse {
    pub qp_num: u32,
    pub doorbell_page: *mut u8,
    pub blueflame_page: *mut u8,
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