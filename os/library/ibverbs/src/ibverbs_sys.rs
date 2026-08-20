//! This crate is a replacement for rdma-core on Linux.
//!
//! The struct definitions are partly taken from the rust-bindgen output.
#![allow(non_camel_case_types)]

extern crate alloc;

use alloc::{boxed::Box, string::{String, ToString}, vec, vec::{Vec}};
use core::mem;
use core::mem::MaybeUninit;
use core3::io::{Error, ErrorKind, Result as Result};
use spin::Mutex;
pub use rdma::{
    __be64, ibv_access_flags, ibv_ah_attr, ibv_device_attr, ibv_gid, ibv_mtu,
    ibv_port_attr, ibv_port_state,
    ibv_qp_attr, ibv_qp_attr_mask, ibv_qp_cap, ibv_qp_state, ibv_qp_type,
    ibv_recv_wr, ibv_send_wr, ibv_send_wr_wr, ibv_send_flags, ibv_sge,
    ibv_wr_opcode, ibv_wc, ibv_wc_opcode, ibv_wc_status,
};
pub(crate) use rdma::ibv_device;
use rdma::uverbs_uapi::{CreateMrRequest, CreateMrResponse, QueryPortRequest, ModifyQpRequest, UVERBS_MAX_QUERY_DEVICES_REQ, ReceiveWorkRequest, UserSlice, SendWorkRequest, UverbsCmd};
use rdma::uverbs_uapi::UverbsCmd::{DeregMr, DestroyCq, DestroyQp, ModifyQp, QueryDevice, QueryDevices, QueryPort, RegMr};
use syscall::return_vals::{Errno, SyscallResult};
use crate::mlx4;

pub fn uverbs(
    device_fd: usize, cmd: UverbsCmd, user_in: UserSlice, user_out: UserSlice,
) -> SyscallResult {
    use syscall::{syscall, SystemCall::Uverb};

    syscall(Uverb, &[
        device_fd,
        cmd as u64 as usize,
        user_in.address as usize,
        user_in.size,
        user_out.address as usize,
        user_out.size,
    ])
}

/// Converts a failed uverbs syscall's `Errno` into an `io::Error` that keeps that errno visible
/// (as opposed to `Error::from(ErrorKind::Other)`, which collapses every failure into the
/// indistinguishable, message-less `Kind(Other)`).
fn uverbs_error(errno: Errno) -> Error {
    let msg = match errno {
        Errno::EUNKN => "uverbs: unknown error",
        Errno::ENOENT => "uverbs: no such file or directory",
        Errno::ENOHANDLES => "uverbs: no more free handles",
        Errno::EBADF => "uverbs: bad file descriptor",
        Errno::EACCES => "uverbs: permission denied",
        Errno::EEXIST => "uverbs: file/directory exists",
        Errno::ENOTDIR => "uverbs: not a directory",
        Errno::EINVAL => "uverbs: invalid argument",
        Errno::EINVALH => "uverbs: invalid handle",
        Errno::ENOTEMPTY => "uverbs: directory not empty",
        Errno::EBADSTR => "uverbs: bad string",
        Errno::EBUSY => "uverbs: device busy",
        Errno::ENOTSUP => "uverbs: operation not supported",
        Errno::ECONNRESET => "uverbs: connection reset by peer",
        Errno::ERDONLY => "uverbs: read-only file system",
        Errno::EAGAIN => "uverbs: resource unavailable",
        Errno::ESRCH => "uverbs: no such thread",
        Errno::EOF => "uverbs: end of file",
        Errno::EPIPE => "uverbs: broken pipe",
        Errno::ENOMEM => "uverbs: not enough space / cannot allocate memory",
        Errno::EISDIR => "uverbs: is a directory",
        Errno::EFAULT => "uverbs: fault occurred",
        Errno::ENOCMD => "uverbs: no such command",
    };
    Error::new(ErrorKind::Other, msg)
}

pub struct ibv_context_ops {
    pub poll_cq: Option<fn(
        &ibv_cq<'_>, &mut [ibv_wc],
    ) -> Result<i32>>,
    /// This is unsafe because the sges contain raw addresses.
    // TODO: figure out a way to return the bad wr
    pub post_send: Option<unsafe fn(
        &mut ibv_qp<'_, '_>, &mut ibv_send_wr,
    ) -> Result<()>>,
    /// This is unsafe because the sges contain raw addresses.
    // TODO: figure out a way to return the bad wr
    pub post_recv: Option<unsafe fn(
        &mut ibv_qp<'_, '_>, &mut ibv_recv_wr,
    ) -> Result<()>>,
}

// These bypass the syscall for the data-path (fast-path), posting/polling directly against
// memory mapped in by `CreateCq`/`CreateQp`; see `crate::mlx4`.
const IBV_CONTEXT_OPS: ibv_context_ops = ibv_context_ops {
    poll_cq: Some(ibv_poll_cq),
    post_send: Some(ibv_post_send),
    post_recv: Some(ibv_post_recv),
};

pub struct ibv_context {
    pub ops: ibv_context_ops,
    device_handle: usize,
    // TODO: add support for different device types, i.e. mlx5, iWARP, RoCE
    mlx4: mlx4::Device,
}

impl ibv_context {
    /// Get access to the underlying device fd.
    fn device_handle(&self) -> usize {
        self.device_handle
    }
}

pub struct ibv_cq<'ctx> {
    context: &'ctx ibv_context,
    number: u32,
    /// Consumer-supplied context returned for completion events
    _cq_context: isize,
    cq: Mutex<mlx4::completion_queue::CompletionQueue>,
}

impl Drop for ibv_cq<'_> {
    fn drop(&mut self) {
        let device_handle = self.context.device_handle();
        uverbs(device_handle, DestroyCq, UserSlice::from_ref(&self.number), UserSlice::EMPTY)
            .expect("failed to destroy completion queue");
    }
}

pub struct ibv_mr<'pd> {
    pd: &'pd ibv_pd<'pd>,
    handle: u32,
    /// virtual address
    pub addr: usize,
    pub length: usize,
    pub lkey: u32,
    pub rkey: u32,
}

impl Drop for ibv_mr<'_> {
    fn drop(&mut self) {
        let device_handle = self.pd.context.device_handle();
        uverbs(device_handle, DeregMr, UserSlice::from_ref(&self.handle), UserSlice::EMPTY)
            .expect("failed to destroy memory region");
    }
}

pub struct ibv_pd<'ctx> {
    context: &'ctx ibv_context,
}

// pub struct ibv_srq {}

pub struct ibv_qp<'ctx, 'cq> {
    pub ops: &'ctx ibv_context_ops,
    pub qp_num: u32,
    send_cq: &'cq ibv_cq<'ctx>,
    recv_cq: &'cq ibv_cq<'ctx>,
}

impl Drop for ibv_qp<'_, '_> {
    fn drop(&mut self) {
        let device_handle = self.send_cq.context.device_handle();
        uverbs(device_handle, DestroyQp, UserSlice::from_ref(&self.qp_num), UserSlice::EMPTY)
            .expect("failed to destroy queue pair");
    }
}

#[allow(dead_code)]
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
    let mut devices : Vec<MaybeUninit<ibv_device>> = vec![MaybeUninit::uninit();UVERBS_MAX_QUERY_DEVICES_REQ];
    match uverbs(0, QueryDevices, UserSlice::EMPTY, UserSlice::from_mut_slice(&mut devices)) {
        Ok(count) => {
            unsafe { devices.set_len(count) };
            Ok(unsafe { mem::transmute::<_,Vec<ibv_device>>(devices) })
        }
        Err(e) => Err(uverbs_error(e))
    }
}

/// Return kernel device name
pub fn ibv_get_device_name(_device: &ibv_device) -> Option<String> {
    // TODO: don't hardcode this
    Some("mlx4_todo".to_string())
}

/// Return kernel device index
///
/// Available for the kernel with support of IB device query
/// over netlink interface. For the unsupported kernels, the
/// relevant error will be returned.
pub fn ibv_get_device_index(device: &ibv_device) -> Result<i32> {
    device.handle.try_into()
        .map_err(|x| Error::from(ErrorKind::InvalidData))
}

/// Return device's node GUID
pub fn ibv_get_device_guid(_device: &ibv_device) -> Result<__be64> {
    todo!()
}


/// Initialize device for use
pub fn ibv_open_device(device: &ibv_device) -> Result<ibv_context> {
    Ok(ibv_context {
        device_handle: device.handle,
        ops: IBV_CONTEXT_OPS,
        mlx4: mlx4::Device::new(device.handle),
    })
}

/// Get device properties
pub fn ibv_query_device(context: &ibv_context) -> Result<ibv_device_attr> {
    let device_handle = context.device_handle();

    let mut resp = MaybeUninit::<ibv_device_attr>::uninit();

    match uverbs(device_handle, QueryDevice, UserSlice::EMPTY, UserSlice::from_mut(&mut resp)) {
        Ok(_) => Ok(unsafe { resp.assume_init() }),
        Err(e) => Err(uverbs_error(e))
    }
}

/// Get port properties
pub fn ibv_query_port(context: &ibv_context, port_num: u8) -> Result<ibv_port_attr> {
    let device_handle = context.device_handle();

    let req = QueryPortRequest {
        port_num
    };

    let mut resp = MaybeUninit::<ibv_port_attr>::uninit();
    match uverbs(device_handle, QueryPort, UserSlice::from_ref(&req), UserSlice::from_mut(&mut resp)) {
        Ok(_) => Ok(unsafe { resp.assume_init() }),
        Err(e) => Err(uverbs_error(e))
    }
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
pub fn ibv_alloc_pd(context: &ibv_context) -> Result<ibv_pd<'_>> {
    // TODO: figure out how to actually do this as the Nautilus driver has no
    // concept of protection domains
    Ok(ibv_pd { context })
}

/// Register a memory region
pub fn ibv_reg_mr<'pd>(
    pd: &'pd ibv_pd<'_>, ptr: *mut u8, len: usize, access: ibv_access_flags,
) -> Result<ibv_mr<'pd>> {
    if len == 0 {
        return Err(Error::from(ErrorKind::InvalidInput))
    }

    let device_handle = pd.context.device_handle();

    let req = CreateMrRequest {
        ibv_access_flags: access,
        data_ptr: ptr,
        len,
    };

    let mut resp = MaybeUninit::<CreateMrResponse>::uninit();

    match uverbs(device_handle, RegMr, UserSlice::from_ref(&req), UserSlice::from_mut(&mut resp)) {
        Ok(_) => {
            let CreateMrResponse { handle, lkey, rkey } = unsafe { resp.assume_init() };
            Ok(ibv_mr { pd, handle, addr: ptr.addr(), length: len, lkey, rkey })
        },
        Err(e) => Err(uverbs_error(e))
    }
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
) -> Result<ibv_cq<'_>> {
    assert!(channel.is_none());
    assert_eq!(comp_vector, 0);

    let cq = mlx4::completion_queue::CompletionQueue::create(context.device_handle(), cqe)
        .map_err(|_| Error::from(ErrorKind::Other))?;
    let number = cq.number();
    Ok(ibv_cq { context, number, _cq_context: cq_context, cq: Mutex::new(cq) })
}

/// Create a queue pair.
pub fn ibv_create_qp<'ctx, 'cq>(
    _pd: &'ctx ibv_pd<'_>, qp_init_attr: &mut ibv_qp_init_attr<'cq, 'ctx>,
) -> Result<ibv_qp<'ctx, 'cq>> {
    let send_cq = qp_init_attr.send_cq;
    let recv_cq = qp_init_attr.recv_cq;
    assert!(core::ptr::eq(send_cq.context, recv_cq.context));

    let qp_num = send_cq.context.mlx4.create_qp(send_cq.number, recv_cq.number, qp_init_attr.qp_type, qp_init_attr.cap)
        .map_err(|_| Error::from(ErrorKind::Other))?;

    Ok(ibv_qp {
        ops: &IBV_CONTEXT_OPS,
        qp_num,
        send_cq,
        recv_cq,
    })
}

/// Modify a queue pair.
pub fn ibv_modify_qp(
    qp: &mut ibv_qp<'_, '_>, attr: &ibv_qp_attr, attr_mask: ibv_qp_attr_mask,
) -> Result<()> {
    let device_handle = qp.recv_cq.context.device_handle();
    let attr = *attr;

    let req = ModifyQpRequest {
        qp_num: qp.qp_num,
        attr,
        attr_mask
    };

    match uverbs(device_handle, ModifyQp, UserSlice::from_ref(&req), UserSlice::EMPTY) {
        Ok(_) => {
            // The kernel just updated its own `QueuePair::state` from this transition; mirror it
            // into the mlx4 registry too, since `post_send`/`post_recv` check the copy there
            // (posting no longer round-trips through the kernel to see this).
            if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_STATE) {
                qp.send_cq.context.mlx4.set_qp_state(qp.qp_num, attr.qp_state)
                    .map_err(|_| Error::from(ErrorKind::Other))?;
            }
            Ok(())
        }
        Err(e) => Err(uverbs_error(e))
    }
}

/// poll a completion queue (CQ)
///
/// This reads directly from the CQE buffer mapped in by `ibv_create_cq` — no syscall per poll.
fn ibv_poll_cq(
    cq: &ibv_cq<'_>, wc: &mut [ibv_wc],
) -> Result<i32> {
    cq.cq.lock().poll(&cq.context.mlx4, wc)
        .map(|count| count.try_into().unwrap())
        .map_err(|_| Error::from(ErrorKind::Other))
}

/// post a list of work requests (WRs) to a send queue
///
/// This posts directly into the WQE buffer mapped in by `ibv_create_qp` and rings the SQ
/// doorbell — no syscall per post.
unsafe fn ibv_post_send(
    qp: &mut ibv_qp<'_, '_>, wr: &mut ibv_send_wr,
) -> Result<()> {
    let mut wrs = Vec::new();
    let mut cur = Some(wr);
    while let Some(wr) = cur {
        wrs.push(SendWorkRequest {
            wr_id: wr.wr_id,
            sges: wr.sg_list.clone(),
            opcode: wr.opcode,
            send_flags: wr.send_flags,
            wr: wr.wr,
        });
        cur = wr.next.as_mut();
    }

    qp.send_cq.context.mlx4.post_send(qp.qp_num, &wrs).map_err(|_| Error::from(ErrorKind::Other))
}

/// post a list of work requests (WRs) to a receive queue
///
/// This posts directly into the WQE buffer mapped in by `ibv_create_qp` and updates the RQ
/// doorbell record — no syscall per post.
unsafe fn ibv_post_recv(
    qp: &mut ibv_qp<'_, '_>, wr: &mut ibv_recv_wr,
) -> Result<()> {
    let mut wrs = Vec::new();
    let mut cur = Some(wr);
    while let Some(wr) = cur {
        wrs.push(ReceiveWorkRequest {
            wr_id: wr.wr_id,
            sges: wr.sg_list.clone(),
        });
        cur = unsafe { wr.next.as_mut() };
    }

    qp.recv_cq.context.mlx4.post_receive(qp.qp_num, &wrs).map_err(|_| Error::from(ErrorKind::Other))
}

pub fn ibv_send_wr_builder(wr_id: u64, opcode: ibv_wr_opcode, send_flags: ibv_send_flags,
    wr: ibv_send_wr_wr, next: *mut ibv_send_wr, sg_list: Vec<ibv_sge>) -> Box<ibv_send_wr> {
    Box::new(ibv_send_wr {
                wr_id,
                next,
                sg_list,
                opcode,
                send_flags,
                wr,
                qp_type: Default::default(),
                __bindgen_anon_1: Default::default(),
                __bindgen_anon_2: Default::default(),
    })
}
