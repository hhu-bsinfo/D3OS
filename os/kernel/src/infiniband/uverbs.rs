use core::{mem::offset_of, slice::from_raw_parts_mut};
use crate::{device::mlx4::{ConnectX3Nic, device_in_range}, security::syscall_sec::copy_from_user};
use alloc::vec;
use rdma::{ibv_device_attr, ibv_port_attr, ibv_qp_attr, ibv_qp_cap, 
    ibv_recv_wr, ibv_send_wr, ibv_wc, 
    uverbs_uapi::{TypeSize, UVERBS_CMD_CREATE_CQ, UVERBS_CMD_CREATE_QP, 
        UVERBS_CMD_DEREGISTER_MR, UVERBS_CMD_DESTROY_CQ, UVERBS_CMD_DESTROY_QP, UVERBS_CMD_MODIFY_QP, 
        UVERBS_CMD_POLL_CQ, UVERBS_CMD_POST_RECV, UVERBS_CMD_POST_SEND, UVERBS_CMD_QUERY_DEVICE, 
        UVERBS_CMD_QUERY_DEVICES, UVERBS_CMD_QUERY_PORT, UVERBS_CMD_REGISTER_MR, UVERBS_MAGIC, 
        UVERBS_MINOR_NOT_PRESENT, UVERBS_MINOR_PRESENT, UverbsCmd, 
        ibv_cq_container, ibv_cq_poll_container, ibv_device_attr_container, 
        ibv_mr_container, ibv_mr_res, ibv_port_attr_container, ibv_qp_container, ibv_qp_modify_container, 
        ibv_qp_post_recv_container, ibv_qp_post_send_container, uverbs_per_cmd_size}};
use super::uverbs_cmd::*;
use crate::security::syscall_sec::{access_ok, copy_to_user};
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
    UVERBS_CMD_DEREGISTER_MR
];

macro_rules! inside_unsafe (
    ($e:expr) => (
        unsafe { $e }
    );
);

macro_rules! inline_cast (
    (mut $v_ref:expr, $s_type:ty, $t_type:ty) => (
        $v_ref as *mut $s_type as *mut $t_type
    );
    ($v_ref:expr, $s_type:ty, $t_type:ty) => (
        $v_ref as *const $s_type as *const $t_type
    );
);

pub fn uverbs_ctl(minor: usize, cmd: usize, arg: usize) -> SyscallResult {
    let UverbsCmd::Call(_, _, size, magic, has_minor) = UverbsCmd::decode(cmd as u64);

    if magic != UVERBS_MAGIC {
        return Err(Errno::EINVAL);
    }

    match has_minor {
        UVERBS_MINOR_PRESENT if !device_in_range(minor) => Err(Errno::EINVAL),
        UVERBS_MINOR_NOT_PRESENT if (minor != 0 || 
            UVERBS_SUPPORTED_MINOR_TABLE.iter()
            .find(|x| **x == cmd)
            .is_some() ) => Err(Errno::EINVAL),
        _ => Ok(0usize)
    }?;

    match cmd {
        UVERBS_CMD_QUERY_DEVICES => {
            let __user_buf = arg as *mut u8;
            let mut __kernel_buf = vec![0usize; 
                uverbs_per_cmd_size(UVERBS_CMD_QUERY_DEVICES) / size_of::<usize>()];

            let query_h = uverbs_query_devices(&mut __kernel_buf);
                
            access_ok(__user_buf.addr(), query_h * size_of::<usize>())?;
            copy_to_user(
                __user_buf, 
                __kernel_buf.as_ptr().cast(), 
                query_h * size_of::<usize>()
            )?;

            Ok(query_h)
        },
        UVERBS_CMD_QUERY_DEVICE => {
            let __user_buf = arg as *mut ibv_device_attr_container;

            access_ok(__user_buf.addr(), size.into())?;

            let __user_port_off = offset_of!(ibv_device_attr_container, phys_port_cnt);
            let __user_fw_off = offset_of!(ibv_device_attr_container, fw_ver);

            let __user_buf_fw_str = inside_unsafe!(__user_buf.cast::<u8>().add(__user_fw_off));
            let __user_buf_port = inside_unsafe!(__user_buf.cast::<u8>().add(__user_port_off));

            let dev_attr = uverbs_query_device(minor).map_err(|_| Errno::EINVAL)?;

            let size_trunc = dev_attr.fw_ver.len().min(ibv_device_attr::S);
            let __kernel_fw_str = dev_attr.fw_ver.as_ptr();

            access_ok(__user_buf_fw_str.addr(), size_trunc);

            copy_to_user(__user_buf_fw_str, __kernel_fw_str, size_trunc)?;
            copy_to_user(__user_buf_port, &dev_attr.phys_port_cnt as *const _, size_of::<u8>())?;

            Ok(size_trunc)
            
        },
        UVERBS_CMD_QUERY_PORT => {
            let __user_buf = arg as *mut ibv_port_attr_container;
            
            let mut __kernel_buf_arr = [0u8; ibv_port_attr_container::S];
            let __kernel_buf = __kernel_buf_arr.as_mut_ptr();

            access_ok(__user_buf.addr(), size.into())?;

            let __user_port_off = offset_of!(ibv_port_attr_container, port_num);
            let __user_port_attr_off = offset_of!(ibv_port_attr_container, ibv_port_attr);

            let __user_buf_port_num = inside_unsafe!(__user_buf.cast::<u8>().add(__user_port_off));
            let __user_buf_port_attr = inside_unsafe!(__user_buf.cast::<u8>().add(__user_port_attr_off));

            copy_from_user(__kernel_buf, __user_buf_port_num, ibv_port_attr_container::S)?;

            let port_num = unsafe { *__kernel_buf };

            let port_attr = uverbs_query_port(minor, port_num).map_err(|_| Errno::EINVAL)?;
            let __kernel_buf_port_attr = inline_cast!(&port_attr, ibv_port_attr, u8);

            copy_to_user(__user_buf_port_attr, __kernel_buf_port_attr, size_of::<ibv_port_attr>())
            
        }
        UVERBS_CMD_REGISTER_MR => {
            let __user_buf = arg as *mut ibv_mr_container;

            let __user_ibv_mr_res_off = offset_of!(ibv_mr_container, ibv_mr_res);
            let __user_ibv_mr_res = inside_unsafe!(__user_buf.cast::<u8>().add(__user_ibv_mr_res_off));

            access_ok(__user_buf.addr(), size.into())?;

            let mut __kernel_ibv_mr_container: ibv_mr_container = Default::default();
           
            let __kernel_copy_size = offset_of!(ibv_mr_container, len) + size_of::<usize>();
        
            copy_from_user(
                inline_cast!(mut &mut __kernel_ibv_mr_container, ibv_mr_container, u8),
                __user_buf.cast(),
                __kernel_copy_size)?;
        
            let supported_len = __kernel_ibv_mr_container.len.min(ibv_mr_container::S);

            access_ok(__kernel_ibv_mr_container.data_ptr.addr(), supported_len)?;

            let __user_data_ref = unsafe { from_raw_parts_mut(
                __kernel_ibv_mr_container.data_ptr, supported_len) };

            let ibv_mr_res = uverbs_register_mem_region(
                minor, 
                __kernel_ibv_mr_container.ibv_access_flags, 
                __user_data_ref
            ).map_err(|_| Errno::EINVAL)?;

            copy_to_user(__user_ibv_mr_res, inline_cast!(&ibv_mr_res, ibv_mr_res, u8), size_of::<ibv_mr_res>())
            
        },
        UVERBS_CMD_CREATE_CQ => {
            let __user_buf = arg as *mut ibv_cq_container;
            let __user_cq_num_off = offset_of!(ibv_cq_container, cq_num);
            let __user_cq_num = inside_unsafe!(__user_buf.cast::<u8>().add(__user_cq_num_off));

            let mut __kernel_cq_container = ibv_cq_container::default();

            access_ok(__user_buf.addr(), size.into())?;

            copy_from_user(inline_cast!(mut &mut __kernel_cq_container, ibv_cq_container, u8), __user_buf.cast(), size.into())?;

            let cq_num_ref = uverbs_create_cq(minor, &mut __kernel_cq_container).map_err(|_| Errno::EINVAL)?;
            
            copy_to_user(__user_cq_num, inline_cast!(cq_num_ref, u32, u8), size_of::<u32>())
        },
        UVERBS_CMD_CREATE_QP => {
            let __user_buf = arg as *mut ibv_qp_container;
            
            let __user_qp_num_off = offset_of!(ibv_qp_container, qp_num);
            let __user_qp_num = inside_unsafe!(__user_buf.cast::<u8>().add(__user_qp_num_off));

            access_ok(__user_buf.addr(), size.into())?;

            let mut __kernel_qp_container = ibv_qp_container::default();
            let mut __kernel_ib_caps = ibv_qp_cap::default();

            copy_from_user(inline_cast!(mut &mut __kernel_qp_container, ibv_qp_container, u8), __user_buf.cast(), size.into())?;

            access_ok(__kernel_qp_container.ib_caps.addr(), ibv_qp_container::S)?;

            copy_from_user(inline_cast!(mut &mut __kernel_ib_caps, ibv_qp_cap, u8), __kernel_qp_container.ib_caps.cast(), ibv_qp_container::S)?;

            __kernel_qp_container.ib_caps = &mut __kernel_ib_caps;

            let qp_num_ref = uverbs_create_qp(minor, &mut __kernel_qp_container).map_err(|_| Errno::EINVAL)?;
            copy_to_user(__user_qp_num, inline_cast!(qp_num_ref, u32, u8), size_of::<u32>())
        },
        UVERBS_CMD_MODIFY_QP => {
            let __user_buf = arg as *mut ibv_qp_modify_container;

            let mut __kernel_qp_modify_container = ibv_qp_modify_container::default();
            let mut __kernel_qp_attr = ibv_qp_attr::default();

            access_ok(__user_buf.addr(), size.into())?;

            copy_from_user(inline_cast!(mut &mut __kernel_qp_modify_container, ibv_qp_modify_container, u8), __user_buf.cast(), size.into())?;

            access_ok(__kernel_qp_modify_container.attr.addr(), ibv_qp_modify_container::S)?;

            copy_from_user(
            inline_cast!(mut &mut __kernel_qp_attr, ibv_qp_attr, u8), 
            __kernel_qp_modify_container.attr.cast(),
            ibv_qp_modify_container::S)?;

            __kernel_qp_modify_container.attr = &mut __kernel_qp_attr;

            let _ = uverbs_modify_qp(minor, __kernel_qp_modify_container).map_err(|_| Errno::EINVAL)?;

            Ok(0)
        },
        UVERBS_CMD_POLL_CQ => {
            let __user_buf = arg as *mut ibv_cq_poll_container;

            access_ok(__user_buf.addr(), size.into())?;

            let mut __kernel_cq_poll_container = ibv_cq_poll_container::default();

            copy_from_user(inline_cast!(mut &mut __kernel_cq_poll_container, ibv_cq_poll_container, u8), __user_buf.cast(), size.into())?;

            let supported_len = __kernel_cq_poll_container.wc_len.min(ibv_cq_poll_container::S);

            access_ok(__kernel_cq_poll_container.wc.addr(), supported_len)?;

            let mut __kernel_wc_buf = vec![ibv_wc::default(); supported_len];

            let wc_count = uverbs_poll_cq(minor, __kernel_cq_poll_container.cq_num, &mut __kernel_wc_buf[..]).map_err(|_| Errno::EINVAL)?;
            
            // still contains user pointer
            copy_to_user(
                __kernel_cq_poll_container.wc.cast(),
                 __kernel_wc_buf.as_ptr().cast(), 
                 wc_count * size_of::<ibv_wc>())?;

            Ok(wc_count)
        },
        // for now we just check the ibv_send_wr struct, not the internal pointers it points to which
        // needs to be done to prevent security issues ! 
        UVERBS_CMD_POST_SEND => {
            let __user_buf = arg as *mut ibv_qp_post_send_container;

            access_ok(__user_buf.addr(), size as usize)?;

            let mut __kernel_ibv_send_container_wr = ibv_qp_post_send_container::default();
            let mut __kernel_ibv_send_wr = ibv_send_wr::default();

            copy_from_user(
                inline_cast!(mut &mut __kernel_ibv_send_container_wr, ibv_qp_post_send_container, u8),
                __user_buf.cast(), 
                size as usize)?;

            access_ok(__kernel_ibv_send_container_wr.ibv_send_wr.addr(), ibv_qp_post_send_container::S)?;

            copy_from_user(
                inline_cast!(mut &mut  __kernel_ibv_send_wr, ibv_send_wr, u8), 
                __kernel_ibv_send_container_wr.ibv_send_wr.cast(), 
                ibv_qp_post_send_container::S)?;
            __kernel_ibv_send_container_wr.ibv_send_wr = &mut __kernel_ibv_send_wr as *mut _;

            // TODO next, sg_list, have to be checked before proceding

            let _ = uverbs_post_send(minor, &__kernel_ibv_send_container_wr).map_err(|_| Errno::EINVAL);

            Ok(0)
        },
        // same as above
        UVERBS_CMD_POST_RECV => {
            let __user_buf = arg as *mut ibv_qp_post_recv_container;

            access_ok(__user_buf.addr(), size as usize)?;

            let mut __kernel_ibv_recv_container_wr = ibv_qp_post_recv_container::default();
            let mut __kernel_ibv_recv_wr = ibv_recv_wr::default();

            copy_from_user(
                inline_cast!(mut &mut __kernel_ibv_recv_container_wr, ibv_qp_post_recv_container, u8), 
                __user_buf.cast(), 
                size as usize)?;

            access_ok(__kernel_ibv_recv_container_wr.ibv_recv_wr.addr(), ibv_qp_post_send_container::S)?;

            copy_from_user(
                inline_cast!(mut &mut  __kernel_ibv_recv_wr, ibv_recv_wr, u8), 
                __kernel_ibv_recv_container_wr.ibv_recv_wr.cast(), 
                ibv_qp_post_send_container::S)?;
            __kernel_ibv_recv_container_wr.ibv_recv_wr = &mut __kernel_ibv_recv_wr as *mut _;

            // TODO next, sg_list, have to be checked before proceding

            let _ = uverbs_post_recv(minor, &__kernel_ibv_recv_container_wr).map_err(|_| Errno::EINVAL);

            Ok(0)
        },
        UVERBS_CMD_DESTROY_CQ => {
            let cq_num = arg as u32;

            let _ = uverbs_destroy(minor, ConnectX3Nic::destroy_cq, cq_num).map_err(|_| Errno::EINVAL)?;

            Ok(0)
        },
        UVERBS_CMD_DESTROY_QP => {
            let qp_num = arg as u32;

            let _ = uverbs_destroy(minor, ConnectX3Nic::destroy_qp, qp_num).map_err(|_| Errno::EINVAL)?;

            Ok(0)
        },
        UVERBS_CMD_DEREGISTER_MR => {
            let mr_index = arg as u32;

            let _ = uverbs_destroy(minor, ConnectX3Nic::destroy_mr, mr_index).map_err(|_| Errno::EINVAL)?;

            Ok(0)
        }
        _ => Err(Errno::ENOCMD)
    }
}