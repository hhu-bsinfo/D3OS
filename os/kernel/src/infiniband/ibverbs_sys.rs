//! This crate is a replacement for rdma-core on Linux.
//! 
//! The struct definitions are partly taken from the rust-bindgen output.
#![allow(non_camel_case_types)]

extern crate alloc;

use alloc::{string::{String, ToString}, vec::Vec};
use core2::io::{Error, ErrorKind, Result as Result};
use spin::{Mutex, MutexGuard};
use crate::device::mlx4::{get_mlx3_nic, ConnectX3Nic};
pub use super::ib_core::{
    __be64, ibv_access_flags, ibv_ah_attr, ibv_device_attr, ibv_gid, ibv_mtu,
    ibv_port_attr, ibv_port_state,
    ibv_qp_attr, ibv_qp_attr_mask, ibv_qp_cap, ibv_qp_state, ibv_qp_type,
    ibv_recv_wr, ibv_send_wr, ibv_send_wr_wr, ibv_send_flags, ibv_sge,
    ibv_wr_opcode, ibv_wc, ibv_wc_opcode, ibv_wc_status,
};

pub struct ibv_context_ops {
    pub poll_cq: Option<fn(
        &ibv_cq, &mut [ibv_wc],
    ) -> Result<i32>>,
    /// This is unsafe because the sges contain raw addresses.
    // TODO: figure out a way to return the bad wr
    pub post_send: Option<unsafe fn(
        &mut ibv_qp, &mut ibv_send_wr,
    ) -> Result<()>>,
    /// This is unsafe because the sges contain raw addresses.
    // TODO: figure out a way to return the bad wr
    pub post_recv: Option<unsafe fn(
        &mut ibv_qp, &mut ibv_recv_wr,
    ) -> Result<()>>,
}

const IBV_CONTEXT_OPS: ibv_context_ops = ibv_context_ops {
    poll_cq: Some(ibv_poll_cq),
    post_send: Some(ibv_post_send),
    post_recv: Some(ibv_post_recv),
};

pub struct ibv_device {
    nic: &'static Mutex<ConnectX3Nic>,
}

pub struct ibv_context {
    pub ops: ibv_context_ops,
    nic: &'static Mutex<ConnectX3Nic>,
}

impl ibv_context {
    /// Get access to the underlying device.
    fn lock(&self) -> MutexGuard<ConnectX3Nic> {
        self.nic.lock()
    }
}

pub struct ibv_cq<'ctx> {
    context: &'ctx ibv_context,
    number: u32,
    /// Consumer-supplied context returned for completion events
    _cq_context: isize,
}

impl Drop for ibv_cq<'_> {
    fn drop(&mut self) {
        self.context
            .lock()
            .destroy_cq(self.number)
            .expect("failed to destroy completion queue")
    }
}

pub struct ibv_mr<'pd> {
    pd: &'pd ibv_pd<'pd>,
    index: u32,
    /// physical address
    pub addr: usize,
    pub length: usize,
    pub lkey: u32,
    pub rkey: u32,
}

impl Drop for ibv_mr<'_> {
    fn drop(&mut self) {
        self.pd
            .context
            .lock()
            .destroy_mr(self.index)
            .expect("failed to destroy memory region")
    }
}

pub struct ibv_pd<'ctx> {
    context: &'ctx ibv_context,
}

pub struct ibv_srq {}

pub struct ibv_qp<'ctx, 'cq> {
    pub ops: &'ctx ibv_context_ops,
    pub qp_num: u32,
    send_cq: &'cq ibv_cq<'ctx>,
    recv_cq: &'cq ibv_cq<'ctx>,
}

impl Drop for ibv_qp<'_, '_> {
    fn drop(&mut self) {
        self.send_cq.context
            .lock()
            .destroy_qp(self.qp_num.try_into().unwrap())
            .expect("failed to destroy queue pair")
    }
}

pub struct ibv_qp_init_attr<'cq, 'ctx> {
    pub qp_context: isize,
    pub send_cq: &'cq ibv_cq<'ctx>,
    pub recv_cq: &'cq ibv_cq<'ctx>,
    pub srq: Option<()>,
    pub cap: ibv_qp_cap,
    pub qp_type: ibv_qp_type::Type,
    pub sq_sig_all: i32,
}

/// Get list of IB devices currently available
/// 
/// Return a array of IB devices.
pub fn ibv_get_device_list() -> Result<Vec<ibv_device>> {
    let mut devices = Vec::new();
    if let Some(mlx3) = get_mlx3_nic() {
        devices.push(ibv_device { nic: &mlx3, });
    }
    Ok(devices)
}

/// Return kernel device name
pub fn ibv_get_device_name(_device: &ibv_device) -> Option<String> {
    // TODO: don't hardcode this
    Some("mlx3_0".to_string())
}

/// Return kernel device index
/// 
/// Available for the kernel with support of IB device query
/// over netlink interface. For the unsupported kernels, the
/// relevant error will be returned.
pub fn ibv_get_device_index(_device: &ibv_device) -> Result<i32> {
    Err(Error::from(ErrorKind::InvalidData))
}

/// Return device's node GUID
pub fn ibv_get_device_guid(_device: &ibv_device) -> Result<__be64> {
    todo!()
}


/// Initialize device for use
pub fn ibv_open_device(device: &ibv_device) -> Result<ibv_context> {
    Ok(ibv_context { nic: device.nic, ops: IBV_CONTEXT_OPS, })
}

/// Get device properties
pub fn ibv_query_device(context: &ibv_context) -> Result<ibv_device_attr> {
    context
        .lock()
        .query_device()
        .map_err(|s| Error::new(ErrorKind::Other, s))
}

/// Get port properties
pub fn ibv_query_port(
    context: &ibv_context, port_num: u8,
) -> Result<ibv_port_attr> {
    context
        .lock()
        .query_port(port_num)
        .map_err(|s| Error::new(ErrorKind::Other, s))
}

/// Get a GID table entry
pub fn ibv_query_gid(
    _context: &ibv_context, _port_num: u8, _index: i32,
) -> Result<ibv_gid> {
    // TODO: figure out how to actually do this as the Nautilus driver can't
    Ok(ibv_gid { raw: [0; 16] })
}

/// Allocate a protection domain
/// 
/// This is currently just a stub.
pub fn ibv_alloc_pd(context: &ibv_context) -> Result<ibv_pd> {
    // TODO: figure out how to actually do this as the Nautilus driver has no
    // concept of protection domains
    Ok(ibv_pd { context })
}

/// Register a memory region
pub fn ibv_reg_mr<'pd, T>(
    pd: &'pd ibv_pd, data: &mut [T], access: ibv_access_flags,
) -> Result<ibv_mr<'pd>> {
    let (index, addr, lkey, rkey) = pd.context
        .lock()
        .create_mr(data, access)
        .map_err(|s| Error::new(ErrorKind::Other, s))?;
    let length = data.len();
    Ok(ibv_mr { pd, index, addr, length, lkey, rkey })
}

/// Create a completion queue
/// 
/// @context - Context CQ will be attached to
/// @cqe - Minimum number of entries required for CQ
/// @cq_context - Consumer-supplied context returned for completion events
/// @channel - Completion channel where completion events will be queued.
///     May be NULL if completion events will not be used.
/// @comp_vector - Completion vector used to signal completion events.
///     Must be >= 0 and < context->num_comp_vectors.
pub fn ibv_create_cq(
    context: &ibv_context, cqe: i32, cq_context: isize,
    channel: Option<()>, comp_vector: i32,
) -> Result<ibv_cq> {
    assert!(channel.is_none());
    assert_eq!(comp_vector, 0);
    let number = context
        .lock()
        .create_cq(cqe)
        .map_err(|s| Error::new(ErrorKind::Other, s))?;
    Ok(ibv_cq { context, number, _cq_context: cq_context, })
}

/// Create a queue pair.
pub fn ibv_create_qp<'ctx, 'cq>(
    pd: &'ctx ibv_pd, qp_init_attr: &mut ibv_qp_init_attr<'cq, 'ctx>,
) -> Result<ibv_qp<'ctx, 'cq>> {
    let send_cq = qp_init_attr.send_cq;
    let recv_cq = qp_init_attr.recv_cq;
    assert!(core::ptr::eq(send_cq.context, recv_cq.context));
    let qp_num = pd.context
        .lock()
        .create_qp(
            qp_init_attr.qp_type, send_cq.number, recv_cq.number,
            &mut qp_init_attr.cap,
        )
        .map_err(|s| Error::new(ErrorKind::Other, s))?
        .try_into().unwrap();
    Ok(ibv_qp { ops: &IBV_CONTEXT_OPS, qp_num, send_cq, recv_cq, })
}

/// Modify a queue pair.
pub fn ibv_modify_qp(
    qp: &mut ibv_qp, attr: &ibv_qp_attr, attr_mask: ibv_qp_attr_mask,
) -> Result<()> {
    qp.recv_cq.context.lock()
        .modify_qp(qp.qp_num, attr, attr_mask)
        .map_err(|s| Error::new(ErrorKind::Other, s))
}

/// poll a completion queue (CQ)
fn ibv_poll_cq(
    cq: &ibv_cq, wc: &mut [ibv_wc],
) -> Result<i32> {
    cq.context.lock()
        .poll_cq(cq.number, wc)
        .map(|num| num.try_into().unwrap())
        .map_err(|s| Error::new(ErrorKind::Other, s))
}

/// post a list of work requests (WRs) to a send queue
unsafe fn ibv_post_send(
    qp: &mut ibv_qp, wr: &mut ibv_send_wr,
) -> Result<()> {
    qp.send_cq.context.lock()
        .post_send(qp.qp_num, wr)
        .map_err(|s| Error::new(ErrorKind::Other, s))
}

/// post a list of work requests (WRs) to a receive queue
unsafe fn ibv_post_recv(
    qp: &mut ibv_qp, wr: &mut ibv_recv_wr,
) -> Result<()> {
    qp.recv_cq.context.lock()
        .post_receive(qp.qp_num, wr)
        .map_err(|s| Error::new(ErrorKind::Other, s))
}
