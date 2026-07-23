use super::uverbs_cmd::*;
use crate::device::mlx4::{device_in_range, ConnectX3Nic};
use crate::process_manager;
use alloc::vec;
use core::ptr::copy_nonoverlapping;
use core::{mem::offset_of, slice::from_raw_parts_mut};
use rdma::uverbs_uapi::UVERBS_MAX_QUERY_DEVICES_REQ;
use rdma::{ibv_device, ibv_device_attr, ibv_port_attr, ibv_qp_attr, ibv_qp_cap, ibv_recv_wr, ibv_send_wr, ibv_wc, uverbs_uapi::{
    ibv_cq_container, ibv_cq_poll_container, ibv_mr_container, ibv_mr_res, ibv_port_attr_container, ibv_qp_container,
    ibv_qp_modify_container, ibv_qp_post_recv_container, ibv_qp_post_send_container, TypeSize, UverbsCmd, UVERBS_CMD_CREATE_CQ,
    UVERBS_CMD_CREATE_QP, UVERBS_CMD_DEREGISTER_MR, UVERBS_CMD_DESTROY_CQ, UVERBS_CMD_DESTROY_QP, UVERBS_CMD_MODIFY_QP, UVERBS_CMD_POLL_CQ,
    UVERBS_CMD_POST_RECV, UVERBS_CMD_POST_SEND, UVERBS_CMD_QUERY_DEVICE, UVERBS_CMD_QUERY_DEVICES, UVERBS_CMD_QUERY_PORT, UVERBS_CMD_REGISTER_MR,
    UVERBS_MAGIC, UVERBS_MINOR_NOT_PRESENT, UVERBS_MINOR_PRESENT,
}};
use syscall::return_vals::{Errno, SyscallResult};

static UVERBS_SUPPORTED_MINOR_TABLE: &[usize] = &[
    UVERBS_CMD_QUERY_DEVICE,
    UVERBS_CMD_QUERY_PORT,
    UVERBS_CMD_REGISTER_MR,
    UVERBS_CMD_CREATE_CQ,
    UVERBS_CMD_CREATE_QP,
    UVERBS_CMD_MODIFY_QP,
    UVERBS_CMD_POLL_CQ,
    UVERBS_CMD_POST_SEND,
    UVERBS_CMD_POST_RECV,
    UVERBS_CMD_DESTROY_CQ,
    UVERBS_CMD_DESTROY_QP,
    UVERBS_CMD_DEREGISTER_MR,
];

pub fn uverbs_ctl(minor: usize, cmd: usize, arg: usize) -> SyscallResult {
    let UverbsCmd::Call(_, _, size, magic, has_minor) = UverbsCmd::decode(cmd as u64);

    let process = process_manager().read().current_process();

    if magic != UVERBS_MAGIC {
        return Err(Errno::EINVAL);
    }

    match has_minor {
        UVERBS_MINOR_PRESENT if !device_in_range(minor) => Err(Errno::EINVAL),
        UVERBS_MINOR_NOT_PRESENT if (minor != 0 || UVERBS_SUPPORTED_MINOR_TABLE.iter().find(|x| **x == cmd).is_some()) => Err(Errno::EINVAL),
        _ => Ok(0usize),
    }?;

    match cmd {
        UVERBS_CMD_QUERY_DEVICES => {
            // TODO pass buf_size a argument?
            let devices = uverbs_query_devices(UVERBS_MAX_QUERY_DEVICES_REQ);
            process.virtual_address_space.copy_to_user(arg as *mut ibv_device, &devices)
                .map(|_| devices.len())
                .map_err(|_| Errno::EINVAL)
        }
        UVERBS_CMD_QUERY_DEVICE => {
            let dev_attr = uverbs_query_device(minor).map_err(|_| Errno::EINVAL)?;
            process.virtual_address_space.copy_to_user(arg as *mut ibv_device_attr, &[dev_attr])
                .map(|val| 0)
                .map_err(|_| Errno::EINVAL)
        }
        UVERBS_CMD_QUERY_PORT => {
            let __user_buf = arg as *mut ibv_port_attr_container;

            let mut __kernel_buf_arr = [0u8; ibv_port_attr_container::S];
            let __kernel_buf = __kernel_buf_arr.as_mut_ptr();


            let __user_port_off = offset_of!(ibv_port_attr_container, port_num);
            let __user_port_attr_off = offset_of!(ibv_port_attr_container, ibv_port_attr);

            let __user_buf_port_num = unsafe { __user_buf.cast::<u8>().add(__user_port_off) };
            let __user_buf_port_attr = unsafe { __user_buf.cast::<u8>().add(__user_port_attr_off) };

            unsafe { copy_nonoverlapping(__user_buf_port_num, __kernel_buf, ibv_port_attr_container::S) };

            let port_num = unsafe { *__kernel_buf };

            let port_attr = uverbs_query_port(minor, port_num).map_err(|_| Errno::EINVAL)?;
            let __kernel_buf_port_attr = &port_attr as *const ibv_port_attr as *const u8;

            unsafe { copy_nonoverlapping(__kernel_buf_port_attr, __user_buf_port_attr, size_of::<ibv_port_attr>()) };

            Ok(0)
        }
        UVERBS_CMD_REGISTER_MR => {
            let __user_buf = arg as *mut ibv_mr_container;

            let __user_ibv_mr_res_off = offset_of!(ibv_mr_container, ibv_mr_res);
            let __user_ibv_mr_res = unsafe { __user_buf.cast::<u8>().add(__user_ibv_mr_res_off) };


            let mut __kernel_ibv_mr_container: ibv_mr_container = Default::default();

            let __kernel_copy_size = offset_of!(ibv_mr_container, len) + size_of::<usize>();

            unsafe { copy_nonoverlapping(
                __user_buf.cast(),
                &mut __kernel_ibv_mr_container as *mut ibv_mr_container as *mut u8,
                __kernel_copy_size,
            ) };

            let supported_len = __kernel_ibv_mr_container.len.min(ibv_mr_container::S);

            let __user_data_ref = unsafe { from_raw_parts_mut(__kernel_ibv_mr_container.data_ptr, supported_len) };

            let ibv_mr_res = uverbs_register_mem_region(minor, __kernel_ibv_mr_container.ibv_access_flags, __user_data_ref).map_err(|_| Errno::EINVAL)?;

            unsafe { copy_nonoverlapping(&ibv_mr_res as *const ibv_mr_res as *const u8, __user_ibv_mr_res, size_of::<ibv_mr_res>()) };

            Ok(0)
        }
        UVERBS_CMD_CREATE_CQ => {
            let __user_buf = arg as *mut ibv_cq_container;
            let __user_cq_num_off = offset_of!(ibv_cq_container, cq_num);
            let __user_cq_num = unsafe { __user_buf.cast::<u8>().add(__user_cq_num_off) };

            let mut __kernel_cq_container = ibv_cq_container::default();

            unsafe { copy_nonoverlapping(
                __user_buf.cast(),
                &mut __kernel_cq_container as *mut ibv_cq_container as *mut u8,
                size.into(),
            ) };

            let cq_num_ref = uverbs_create_cq(minor, &mut __kernel_cq_container).map_err(|_| Errno::EINVAL)?;

            unsafe { copy_nonoverlapping(cq_num_ref as *const u32 as *const u8,__user_cq_num,  size_of::<u32>()) };
            Ok(0)
        }
        UVERBS_CMD_CREATE_QP => {
            let __user_buf = arg as *mut ibv_qp_container;

            let __user_qp_num_off = offset_of!(ibv_qp_container, qp_num);
            let __user_qp_num = unsafe { __user_buf.cast::<u8>().add(__user_qp_num_off) };

            let mut __kernel_qp_container = ibv_qp_container::default();

            unsafe { copy_nonoverlapping(
                __user_buf.cast(),
                &mut __kernel_qp_container as *mut ibv_qp_container as *mut u8,
                size.into(),
            ) };

            let qp_num_ref = uverbs_create_qp(minor, &mut __kernel_qp_container).map_err(|_| Errno::EINVAL)?;
            unsafe { copy_nonoverlapping(qp_num_ref as *const u32 as *const u8, __user_qp_num, size_of::<u32>()) };
            Ok(0)
        }
        UVERBS_CMD_MODIFY_QP => {
            let __user_buf = arg as *mut ibv_qp_modify_container;

            let mut __kernel_qp_modify_container = ibv_qp_modify_container::default();
            let mut __kernel_qp_attr = ibv_qp_attr::default();

            unsafe { copy_nonoverlapping(
                __user_buf.cast(),
                &mut __kernel_qp_modify_container as *mut ibv_qp_modify_container as *mut u8,
                size.into(),
            ) };

            unsafe { copy_nonoverlapping(
                __kernel_qp_modify_container.attr.cast(),
                &mut __kernel_qp_attr as *mut ibv_qp_attr as *mut u8,
                ibv_qp_modify_container::S,
            ) };

            __kernel_qp_modify_container.attr = &mut __kernel_qp_attr;

            let _ = uverbs_modify_qp(minor, __kernel_qp_modify_container).map_err(|_| Errno::EINVAL)?;

            Ok(0)
        }
        UVERBS_CMD_POLL_CQ => {
            let __user_buf = arg as *mut ibv_cq_poll_container;

            let mut __kernel_cq_poll_container = ibv_cq_poll_container::default();

            unsafe { copy_nonoverlapping(
                __user_buf.cast(),
                &mut __kernel_cq_poll_container as *mut ibv_cq_poll_container as *mut u8,
                size.into(),
            ) };

            let supported_len = __kernel_cq_poll_container.wc_len.min(ibv_cq_poll_container::S);

            let mut __kernel_wc_buf = vec![ibv_wc::default(); supported_len];

            let wc_count = uverbs_poll_cq(minor, __kernel_cq_poll_container.cq_num, &mut __kernel_wc_buf[..]).map_err(|_| Errno::EINVAL)?;

            // still contains user pointer
            unsafe { copy_nonoverlapping(
                __kernel_wc_buf.as_ptr() as *const u8,
                __kernel_cq_poll_container.wc as *mut u8,
                wc_count * size_of::<ibv_wc>(),
            ) };

            Ok(wc_count)
        }
        // for now we just check the ibv_send_wr struct, not the internal pointers it points to which
        // needs to be done to prevent security issues !
        UVERBS_CMD_POST_SEND => {
            let __user_buf = arg as *mut ibv_qp_post_send_container;

            let mut __kernel_ibv_send_container_wr = ibv_qp_post_send_container::default();
            let mut __kernel_ibv_send_wr = ibv_send_wr::default();

            unsafe { copy_nonoverlapping(
                __user_buf.cast(),
                &mut __kernel_ibv_send_container_wr as *mut ibv_qp_post_send_container as *mut u8,
                size as usize,
            ) };

            unsafe { copy_nonoverlapping(
                __kernel_ibv_send_container_wr.ibv_send_wr.cast(),
                &mut  __kernel_ibv_send_wr as *mut ibv_send_wr as *mut u8,
                ibv_qp_post_send_container::S,
            ) };
            __kernel_ibv_send_container_wr.ibv_send_wr = &mut __kernel_ibv_send_wr as *mut _;

            // TODO next, sg_list, have to be checked before proceding

            let _ = uverbs_post_send(minor, &__kernel_ibv_send_container_wr).map_err(|_| Errno::EINVAL);

            Ok(0)
        }
        // same as above
        UVERBS_CMD_POST_RECV => {
            let __user_buf = arg as *mut ibv_qp_post_recv_container;

            let mut __kernel_ibv_recv_container_wr = ibv_qp_post_recv_container::default();
            let mut __kernel_ibv_recv_wr = ibv_recv_wr::default();

            unsafe { copy_nonoverlapping(
                __user_buf.cast(),
                &mut __kernel_ibv_recv_container_wr as *mut ibv_qp_post_recv_container as *mut u8,
                size as usize,
            ) };

            unsafe { copy_nonoverlapping(
                __kernel_ibv_recv_container_wr.ibv_recv_wr.cast(),
                &mut  __kernel_ibv_recv_wr as *mut ibv_recv_wr as *mut u8,
                ibv_qp_post_send_container::S,
            ) };
            __kernel_ibv_recv_container_wr.ibv_recv_wr = &mut __kernel_ibv_recv_wr as *mut _;

            // TODO next, sg_list, have to be checked before proceding

            let _ = uverbs_post_recv(minor, &__kernel_ibv_recv_container_wr).map_err(|_| Errno::EINVAL);

            Ok(0)
        }
        UVERBS_CMD_DESTROY_CQ => {
            let cq_num = arg as u32;

            let _ = uverbs_destroy(minor, ConnectX3Nic::destroy_cq, cq_num).map_err(|_| Errno::EINVAL)?;

            Ok(0)
        }
        UVERBS_CMD_DESTROY_QP => {
            let qp_num = arg as u32;

            let _ = uverbs_destroy(minor, ConnectX3Nic::destroy_qp, qp_num).map_err(|_| Errno::EINVAL)?;

            Ok(0)
        }
        UVERBS_CMD_DEREGISTER_MR => {
            let mr_index = arg as u32;

            let _ = uverbs_destroy(minor, ConnectX3Nic::destroy_mr, mr_index).map_err(|_| Errno::EINVAL)?;

            Ok(0)
        }
        _ => Err(Errno::ENOCMD),
    }
}
