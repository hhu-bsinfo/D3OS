//! This crate is a replacement for rdma-core on Linux.
//! 
//! The struct definitions are partly taken from the rust-bindgen output.
#![allow(non_camel_case_types)]

extern crate alloc;

use alloc::{boxed::Box, string::{String, ToString}, vec, vec::{Vec}};
use core2::io::{Error, ErrorKind, Result as Result};
pub use rdma::{
    __be64, ibv_access_flags, ibv_ah_attr, ibv_device_attr, ibv_gid, ibv_mtu,
    ibv_port_attr, ibv_port_state,
    ibv_qp_attr, ibv_qp_attr_mask, ibv_qp_cap, ibv_qp_state, ibv_qp_type,
    ibv_recv_wr, ibv_send_wr, ibv_send_wr_wr, ibv_send_flags, ibv_sge,
    ibv_wr_opcode, ibv_wc, ibv_wc_opcode, ibv_wc_status,
};

use syscall::{syscall, SystemCall::Uverb};
use rdma::uverbs_uapi::{TypeSize, UVERBS_CMD_CREATE_CQ, UVERBS_CMD_CREATE_QP, UVERBS_CMD_DEREGISTER_MR, UVERBS_CMD_DESTROY_CQ, UVERBS_CMD_DESTROY_QP, UVERBS_CMD_MODIFY_QP, UVERBS_CMD_POLL_CQ, UVERBS_CMD_POST_SEND, UVERBS_CMD_QUERY_DEVICE, UVERBS_CMD_QUERY_DEVICES, UVERBS_CMD_QUERY_PORT, UVERBS_CMD_REGISTER_MR, ibv_cq_container, ibv_cq_poll_container, ibv_device_attr_container, ibv_mr_container, ibv_mr_res, ibv_port_attr_container, ibv_qp_container, ibv_qp_modify_container, ibv_qp_post_recv_container, ibv_qp_post_send_container, uverbs_per_cmd_size};

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
    nic: usize,
}

pub struct ibv_context {
    pub ops: ibv_context_ops,
    nic: usize,
}

impl ibv_context {
    /// Get access to the underlying device fd.
    fn lock(&self) -> usize {
        self.nic
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
        let dev_fd = self.context.lock();

        syscall(Uverb, &[
            dev_fd,
            UVERBS_CMD_DESTROY_CQ,
            self.number as usize
        ]).expect("failed to destroy completion queue");
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
        let dev_fd = self.pd.context.lock();

        syscall(Uverb, &[
            dev_fd,
            UVERBS_CMD_DEREGISTER_MR,
            self.index as usize
        ]).expect("failed to destroy memory region");
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
        let dev_fd = self.send_cq.context.lock();

        syscall(Uverb, &[
            dev_fd,
            UVERBS_CMD_DESTROY_QP,
            self.qp_num as usize
        ]).expect("failed to destroy queue pair");
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
    let cmd_s = uverbs_per_cmd_size(UVERBS_CMD_QUERY_DEVICES) / size_of::<usize>();
    let mut devices_fd = vec![0usize, cmd_s];

    let mut devices : Vec<ibv_device> = Vec::new(); 
    
    let buf_addr = devices_fd.as_mut_ptr().addr();
    
    if let Ok(device_c) = syscall(Uverb, &[0, UVERBS_CMD_QUERY_DEVICES, buf_addr]) {
        for i in 0..device_c {
            devices.push(ibv_device { nic: devices_fd[i] });
        }
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
    let dev_fd = context.lock();

    let dev_attr_container = ibv_device_attr_container {
        fw_ver: [b'0'; ibv_device_attr::S],
        phys_port_cnt: Default::default() 
    };

    match syscall(Uverb, &[
        dev_fd, 
        UVERBS_CMD_QUERY_DEVICE, 
        (&dev_attr_container as *const ibv_device_attr_container).addr()
        ]) {
        Ok(str_s) => {
            let fw_str = String::from_utf8_lossy(&dev_attr_container.fw_ver[..str_s]).into_owned();
            let dev_attr = ibv_device_attr {
                fw_ver: fw_str,
                phys_port_cnt: dev_attr_container.phys_port_cnt
            };
            
            Ok(dev_attr)
        },
        Err(_) => Err(Error::from(ErrorKind::Other))
    }
}

/// Get port properties
pub fn ibv_query_port(
    context: &ibv_context, port_num: u8,
) -> Result<ibv_port_attr> {
    let dev_fd = context.lock();

    let ibv_port_container = ibv_port_attr_container {
        ibv_port_attr: Default::default(),
        port_num
    };

    match syscall(Uverb, &[
            dev_fd, 
            UVERBS_CMD_QUERY_PORT, 
            ((&ibv_port_container) as *const ibv_port_attr_container).addr()
        ]) {
        Ok(_) => Ok(ibv_port_container.ibv_port_attr),
        Err(_) => Err(Error::from(ErrorKind::Other))
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
pub fn ibv_reg_mr<'pd, T>(
    pd: &'pd ibv_pd, data: &mut [T], access: ibv_access_flags,
) -> Result<ibv_mr<'pd>> {
    let data_u8 = data.as_mut_ptr().cast::<u8>();

    let dev_fd = pd.context.lock();

    let ibv_mr_container = ibv_mr_container {
        ibv_access_flags: access,
        data_ptr: data_u8,
        len: data.len(),
        ibv_mr_res: ibv_mr_res {
            index: Default::default(),
            addr:  Default::default(),
            lkey:  Default::default(),
            rkey:  Default::default()
        }
    };

    match syscall(Uverb, &[
        dev_fd,
        UVERBS_CMD_REGISTER_MR,
        (&ibv_mr_container as *const ibv_mr_container).addr()
    ]) {
        Ok(_) => {
            let ibv_mr_res { index, addr, lkey, rkey } = ibv_mr_container.ibv_mr_res;
            let length = data.len();

            Ok(ibv_mr { pd, index, addr, length, lkey, rkey })
        },
        Err(_) => Err(Error::from(ErrorKind::Other))
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
) -> Result<ibv_cq> {
    assert!(channel.is_none());
    assert_eq!(comp_vector, 0);

    let dev_fd = context.lock();

    let ibv_cq_container = ibv_cq_container {
        cq_entries: cqe,
        cq_num: Default::default()
    };

    match syscall(Uverb, &[
        dev_fd,
        UVERBS_CMD_CREATE_CQ,
        (&ibv_cq_container as *const ibv_cq_container).addr()
    ]) {
        Ok(_) => {
            Ok( ibv_cq { context, number: ibv_cq_container.cq_num, _cq_context: cq_context, } )
        },
        Err(_) => Err(Error::from(ErrorKind::Other))
    }
}

/// Create a queue pair.
pub fn ibv_create_qp<'ctx, 'cq>(
    pd: &'ctx ibv_pd, qp_init_attr: &mut ibv_qp_init_attr<'cq, 'ctx>,
) -> Result<ibv_qp<'ctx, 'cq>> {
    let send_cq = qp_init_attr.send_cq;
    let recv_cq = qp_init_attr.recv_cq;
    assert!(core::ptr::eq(send_cq.context, recv_cq.context));

    let dev_fd = pd.context.lock();

    let ibv_qp_container = ibv_qp_container {
        qp_type: qp_init_attr.qp_type,
        send_cq_num: send_cq.number,
        recv_cq_num: recv_cq.number,
        ib_caps: &mut qp_init_attr.cap,
        qp_num: Default::default()
    };

    match syscall(Uverb, &[
        dev_fd,
        UVERBS_CMD_CREATE_QP,
        (&ibv_qp_container as *const ibv_qp_container).addr()
    ]) {
        Ok(_) => {
            Ok(ibv_qp { 
                ops: &IBV_CONTEXT_OPS, 
                qp_num: ibv_qp_container.qp_num, 
                send_cq, 
                recv_cq, 
            })
        },
        Err(_) => Err(Error::from(ErrorKind::Other))
    }
    
}

/// Modify a queue pair.
pub fn ibv_modify_qp(
    qp: &mut ibv_qp, attr: &ibv_qp_attr, attr_mask: ibv_qp_attr_mask,
) -> Result<()> {
    let dev_fd = qp.recv_cq.context.lock();

    let ibv_qp_modify_container = ibv_qp_modify_container {
        qp_num: qp.qp_num, 
        attr, 
        attr_mask
    };

    match syscall(Uverb, &[
        dev_fd,
        UVERBS_CMD_MODIFY_QP,
        (&ibv_qp_modify_container as *const ibv_qp_modify_container).addr()
    ]) {
        Ok(_) => Ok(()),
        Err(_) => Err(Error::from(ErrorKind::Other))
    }
}

/// poll a completion queue (CQ)
fn ibv_poll_cq(
    cq: &ibv_cq<'_>, wc: &mut [ibv_wc],
) -> Result<i32> {
    let dev_fd = cq.context.lock();

    let ibv_cq_poll_container = ibv_cq_poll_container {
        wc: wc.as_mut_ptr(),
        wc_len: wc.len(),
        cq_num: cq.number,
    };

    match syscall(Uverb, &[
        dev_fd,
        UVERBS_CMD_POLL_CQ,
        (&ibv_cq_poll_container as *const ibv_cq_poll_container).addr()
    ]) {
        Ok(wc_count) => Ok(wc_count.try_into().unwrap()),
        Err(_) => Err(Error::from(ErrorKind::Other))
    }
}

/// post a list of work requests (WRs) to a send queue
unsafe fn ibv_post_send(
    qp: &mut ibv_qp, wr: &mut ibv_send_wr,
) -> Result<()> {
    let dev_fd = qp.send_cq.context.lock();

    let ibv_send_wr_container = ibv_qp_post_send_container {
        ibv_send_wr: wr,
        qp_num: qp.qp_num 
    };

    match syscall(Uverb, &[
        dev_fd,
        UVERBS_CMD_POST_SEND,
        (&ibv_send_wr_container as *const ibv_qp_post_send_container).addr()
    ]) {
        Ok(_) => Ok(()),
        Err(_) => Err(Error::from(ErrorKind::Other))
    }
}

/// post a list of work requests (WRs) to a receive queue
unsafe fn ibv_post_recv(
    qp: &mut ibv_qp, wr: &mut ibv_recv_wr,
) -> Result<()> {
    let dev_fd = qp.recv_cq.context.lock();

    let ibv_recv_wr_container = ibv_qp_post_recv_container {
        ibv_recv_wr: wr,
        qp_num: qp.qp_num 
    };

    match syscall(Uverb, &[
        dev_fd,
        UVERBS_CMD_POST_SEND,
        (&ibv_recv_wr_container as *const ibv_qp_post_recv_container).addr()
    ]) {
        Ok(_) => Ok(()),
        Err(_) => Err(Error::from(ErrorKind::Other))
    }
}

pub fn ibv_send_wr_builder(wr_id: u64, opcode: ibv_wr_opcode, send_flags: ibv_send_flags,
    wr: ibv_send_wr_wr, next: *mut ibv_send_wr, sg_list: Vec<ibv_sge>) -> Box<ibv_send_wr> {
    let num_sge = sg_list.len() as i32;
    Box::new(ibv_send_wr {
                wr_id,
                next,
                sg_list,
                num_sge,
                opcode,
                send_flags,
                wr,
                qp_type: Default::default(),
                __bindgen_anon_1: Default::default(),
                __bindgen_anon_2: Default::default(),
    })
}
