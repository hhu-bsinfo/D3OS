use crate::cmd::uverbs;
use crate::cq::WorkCompletion;
use crate::device::Device;
use crate::mr::MemoryRegionMetadata;
use crate::{ReceiveWorkRequest, SendWorkRequest};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::mem;
use core::mem::MaybeUninit;
use core3::io;
use rdma::ib_core::{AccessFlags, DeviceAttr, PortAttr, QueuePairAttr, QueuePairAttrMask, QueuePairCapabilities};
use rdma::uverbs_uapi::UverbsCmd::QueryDevices;
use rdma::uverbs_uapi::{UserSlice, UVERBS_MAX_QUERY_DEVICES_REQ};
use rdma::ProtectionDomainHandle;
use rdma::{DeviceHandle, Gid, QueuePairType};
use spin::RwLock;

mod mlx4;

/// Get all currently available InfiniBand devices
pub fn get_available_devices() -> io::Result<Vec<DeviceHandle>> {
    let mut devices : Vec<MaybeUninit<DeviceHandle>> = vec![MaybeUninit::uninit(); UVERBS_MAX_QUERY_DEVICES_REQ];
    // Safety: DeviceHandle uverbs returns the number of devices and a MaybeUninit<T> has the same layout as T
    uverbs(0, QueryDevices, UserSlice::EMPTY, UserSlice::from_mut_slice(&mut devices)).map(|count| unsafe {
        devices.set_len(count);
        mem::transmute::<_,Vec<DeviceHandle>>(devices)
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


pub trait IbvQueuePair {
    fn number(&self) -> u32;
    /// This is unsafe because the sges contain raw addresses.
    // TODO: figure out a way to return the bad wr
    unsafe fn post_receive(&mut self, wrs: &[&ReceiveWorkRequest]) -> io::Result<()>;
    /// This is unsafe because the sges contain raw addresses.
    // TODO: figure out a way to return the bad wr
    unsafe fn post_send(&mut self, wrs: &[&SendWorkRequest]) -> io::Result<()>;
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
    fn create_qp(self: Arc<Self>, pd: ProtectionDomainHandle, attr: &QpInitAttr) -> io::Result<Arc<RwLock<dyn IbvQueuePair>>>;
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

