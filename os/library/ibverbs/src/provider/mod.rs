use crate::cmd::uverbs;
use crate::{Gid, MemoryRegionMetadata};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use bincode::{Decode, Encode};
use core::mem;
use core::mem::MaybeUninit;
use core3::io;
use rdma::ib_core::{PortAttr, QueuePairAttr, QueuePairAttrMask, SendFlags, SendWorkRequestData, ScatterGatherEntry, AccessFlags, Device, DeviceAttr, QueuePairCapabilities};
use rdma::QueuePairType;
use rdma::ProtectionDomainHandle;
use rdma::uverbs_uapi::UverbsCmd::QueryDevices;
use rdma::uverbs_uapi::{UserSlice, UVERBS_MAX_QUERY_DEVICES_REQ};
use crate::completion_queue::WorkCompletion;
use crate::queue_pair::WorkRequestOpcode;

mod mlx4;

/// Get all currently available InfiniBand devices
pub fn get_available_devices() -> io::Result<Vec<Device>> {
    let mut devices : Vec<MaybeUninit<Device>> = vec![MaybeUninit::uninit(); UVERBS_MAX_QUERY_DEVICES_REQ];
    uverbs(0, QueryDevices, UserSlice::EMPTY, UserSlice::from_mut_slice(&mut devices)).map(|count| unsafe {
        devices.set_len(count);
        mem::transmute::<_,Vec<Device>>(devices)
    })
}

pub fn open_device(device: &Device) -> io::Result<Box<dyn IbvContext>> {
    let device = mlx4::Mlx4Context::new(device.handle)?;
    Ok(Box::new(device))
}

/// Return kernel device name
pub fn get_device_name(_device: &Device) -> Option<&str> {
    // TODO: don't hardcode device name to mlx4
    Some("mlx4_todo")
}

#[repr(C)]
#[derive(Clone, Encode, Decode)]
pub struct SendWorkRequest {
    pub wr_id: u64,
    pub sges: Vec<ScatterGatherEntry>,
    pub opcode: WorkRequestOpcode,
    pub send_flags: SendFlags,
    pub wr: SendWorkRequestData,
}


#[derive(Clone, Encode, Decode)]
pub struct ReceiveWorkRequest {
    pub wr_id: u64,
    pub sges: Vec<ScatterGatherEntry>,
}

pub trait IbvQueuePair {
    fn number(&self) -> u32;
    /// This is unsafe because the sges contain raw addresses.
    // TODO: figure out a way to return the bad wr
    unsafe fn post_receive(&self, wrs: &[ReceiveWorkRequest]) -> io::Result<()>;
    /// This is unsafe because the sges contain raw addresses.
    // TODO: figure out a way to return the bad wr
    unsafe fn post_send(&self, wrs: &[SendWorkRequest]) -> io::Result<()>;
    fn modify(&self, attr: &QueuePairAttr, attr_mask: QueuePairAttrMask) -> io::Result<()>;
}

pub trait IbvCompletionQueue {
    fn number(&self) -> u32;
    fn poll(&self, wcs: &mut [WorkCompletion]) -> io::Result<usize>;
}

pub trait IbvContext {
    fn query_device(&self) -> io::Result<DeviceAttr>;
    fn query_port(&self, port_num: u8) -> io::Result<PortAttr>;
    fn query_gid(&self, port_num: u8, index: i32) -> io::Result<Gid>;

    // --- Queue Pair ---
    fn create_qp(self: Arc<Self>, pd: ProtectionDomainHandle, attr: &QpInitAttr) -> io::Result<Arc<dyn IbvQueuePair>>;
    //fn query_qp();

    // --- Completion Queue ---
    fn create_cq(self: Arc<Self>, min_cpe: i32, cq_context: isize, channel: Option<()>, comp_vector: i32) -> io::Result<Box<dyn IbvCompletionQueue>>;

    // --- Protection Domain ---
    fn alloc_pd(&self) -> io::Result<ProtectionDomainHandle>;
    fn dealloc_pd(&self, pd: ProtectionDomainHandle) -> io::Result<()>;

    // --- Memory Region ---
    fn reg_mr(&self, pd: ProtectionDomainHandle, ptr: *mut u8, len: usize, access: AccessFlags) -> io::Result<MemoryRegionMetadata>;
    fn dereg_mr(&self, meta: MemoryRegionMetadata);
}

pub struct QpInitAttr<'cq> {
    pub qp_context: isize,
    pub send_cq: &'cq dyn IbvCompletionQueue,
    pub recv_cq: &'cq dyn IbvCompletionQueue,
    pub srq: Option<()>,
    pub cap: QueuePairCapabilities,
    pub qp_type: QueuePairType,
    pub sq_sig_all: i32,
}

