use crate::cmd::uverbs;
use crate::MemoryRegionMetadata;
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use bincode::{Decode, Encode};
use core::mem;
use core::mem::MaybeUninit;
use core3::io;
use rdma::ib_core::{ibv_access_flags, ibv_device, ibv_device_attr, ibv_gid, ibv_port_attr, ibv_qp_attr, ibv_qp_attr_mask, ibv_qp_cap, ibv_qp_type, ibv_send_flags, ibv_send_wr_wr, ibv_sge, ibv_wc, ibv_wr_opcode};
use rdma::uverbs_uapi::UverbsCmd::QueryDevices;
use rdma::uverbs_uapi::{UserSlice, UVERBS_MAX_QUERY_DEVICES_REQ};

mod mlx4;

/// Get all currently available InfiniBand devices
pub fn get_available_devices() -> io::Result<Vec<ibv_device>> {
    let mut devices : Vec<MaybeUninit<ibv_device>> = vec![MaybeUninit::uninit();UVERBS_MAX_QUERY_DEVICES_REQ];
    uverbs(0, QueryDevices, UserSlice::EMPTY, UserSlice::from_mut_slice(&mut devices)).map(|count| unsafe {
        devices.set_len(count);
        mem::transmute::<_,Vec<ibv_device>>(devices)
    })
}

pub fn open_device(device: &ibv_device) -> io::Result<Box<dyn IbvContext>> {
    Ok(Box::new(mlx4::Mlx4Context::new(device.handle)))
}

/// Return kernel device name
pub fn get_device_name(_device: &ibv_device) -> Option<&str> {
    // TODO: don't hardcode device name to mlx4
    Some("mlx4_todo")
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


#[derive(Clone, Encode, Decode)]
pub struct ReceiveWorkRequest {
    pub wr_id: u64,
    pub sges: Vec<ibv_sge>,
}

pub trait IbvQueuePair {
    fn number(&self) -> u32;
    /// This is unsafe because the sges contain raw addresses.
    // TODO: figure out a way to return the bad wr
    unsafe fn post_receive(&self, wrs: &[ReceiveWorkRequest]) -> io::Result<()>;
    /// This is unsafe because the sges contain raw addresses.
    // TODO: figure out a way to return the bad wr
    unsafe fn post_send(&self, wrs: &[SendWorkRequest]) -> io::Result<()>;
    fn modify(&self, attr: &ibv_qp_attr, attr_mask: ibv_qp_attr_mask) -> io::Result<()>;
}

pub trait IbvCompletionQueue {
    fn number(&self) -> u32;
    fn poll(&self, wcs: &mut [ibv_wc]) -> io::Result<usize>;
}

pub trait IbvContext {
    fn query_device(&self) -> io::Result<ibv_device_attr>;
    fn query_port(&self, port_num: u8) -> io::Result<ibv_port_attr>;
    fn query_gid(&self, port_num: u8, index: i32) -> io::Result<ibv_gid>;

    // --- Queue Pair ---
    fn create_qp(self: Arc<Self>, attr: &QpInitAttr) -> io::Result<Arc<dyn IbvQueuePair>>;
    //fn query_qp();

    // --- Completion Queue ---
    fn create_cq(self: Arc<Self>, min_cpe: i32, cq_context: isize, channel: Option<()>, comp_vector: i32) -> io::Result<Box<dyn IbvCompletionQueue>>;

    // --- Protection Domain ---
    //fn alloc_pd();
    //fn delalloc_pd();

    // --- Memory Region ---
    fn reg_mr(&self, ptr: *mut u8, len: usize, access: ibv_access_flags) -> io::Result<MemoryRegionMetadata>;
    fn dereg_mr(&self, meta: MemoryRegionMetadata);
}

pub struct QpInitAttr<'cq> {
    pub qp_context: isize,
    pub send_cq: &'cq dyn IbvCompletionQueue,
    pub recv_cq: &'cq dyn IbvCompletionQueue,
    pub srq: Option<()>,
    pub cap: ibv_qp_cap,
    pub qp_type: ibv_qp_type::Type,
    pub sq_sig_all: i32,
}

