use super::uverbs_cmd::*;
use crate::device::mlx4::{device_in_range, ConnectX3Nic};
use crate::process_manager;
use alloc::vec;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::slice::from_raw_parts_mut;
use log::debug;
use rdma::uverbs_uapi::{PostReceiveRequest, QueryPortRequest, UserMemory, UVERBS_MAX_USER_WC_REQ};
use rdma::{ibv_device, ibv_wc, uverbs_uapi::{
    CreateCqRequest, CreateMrRequest, CreateQpRequest, ModifyQpRequest,
    PollCqRequest, PostSendRequest, UverbsCmd, UVERBS_CMD_CREATE_CQ,
    UVERBS_CMD_CREATE_QP, UVERBS_CMD_DEREGISTER_MR, UVERBS_CMD_DESTROY_CQ, UVERBS_CMD_DESTROY_QP, UVERBS_CMD_MODIFY_QP, UVERBS_CMD_POLL_CQ,
    UVERBS_CMD_POST_RECV, UVERBS_CMD_POST_SEND, UVERBS_CMD_QUERY_DEVICE, UVERBS_CMD_QUERY_DEVICES, UVERBS_CMD_QUERY_PORT, UVERBS_CMD_REGISTER_MR,
    UVERBS_MAGIC, UVERBS_MINOR_NOT_PRESENT, UVERBS_MINOR_PRESENT,
}};
use syscall::return_vals::{Errno, SyscallResult};
use x86_64::VirtAddr;

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

/// user_in is a pointer to the parameters provided by the user
/// user_out points to a user buffer for return values
pub fn uverbs_ctl(minor: usize, cmd: usize, user_memory_addr: *const UserMemory) -> SyscallResult {
    let UverbsCmd::Call(_, _, _, magic, has_minor) = UverbsCmd::decode(cmd as u64);
    debug!("Uverbs syscall: {}, {}", minor, cmd);

    let process = process_manager().read().current_process();
    let mut user_memory = MaybeUninit::<UserMemory>::uninit();
    unsafe { process.virtual_address_space.copy_bytes_from_user(user_memory.as_mut_ptr() as *mut u8, VirtAddr::from_ptr(user_memory_addr), size_of::<UserMemory>()) }
        .map_err(|e| Errno::EINVAL)?;
    let user_memory: UserMemory = unsafe { user_memory.assume_init() };
    debug!("Uverbs user memory: {:?}", user_memory);

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
            let devices = uverbs_query_devices(user_memory.out_size as usize / size_of::<ibv_device>());
            copy_slice_to_user(user_memory, &devices)
        }
        UVERBS_CMD_QUERY_DEVICE => {
            let dev_attr = uverbs_query_device(minor).map_err(|_| Errno::EINVAL)?;
            copy_to_user(user_memory, &dev_attr)
        }
        UVERBS_CMD_QUERY_PORT => {
            let mut req: QueryPortRequest = copy_from_user(user_memory)?;
            let port_attr = uverbs_query_port(minor, req.port_num).map_err(|_| Errno::EINVAL)?;
            copy_to_user(user_memory, &port_attr)
        }
        UVERBS_CMD_REGISTER_MR => {
            let mut req: CreateMrRequest = copy_from_user(user_memory)?;
            // todo: use a custom type like UserSlice instead of slice
            let user_slice = unsafe { from_raw_parts_mut(req.data_ptr, req.len) };
            let resp = uverbs_register_mem_region(minor, req.ibv_access_flags, user_slice).map_err(|_| Errno::EINVAL)?;
            copy_to_user(user_memory, &resp)
        }
        UVERBS_CMD_CREATE_CQ => {
            let req: CreateCqRequest = copy_from_user(user_memory)?;
            let resp = uverbs_create_cq(minor, &req).map_err(|_| Errno::EINVAL)?;
            copy_to_user(user_memory, &resp)
        }
        UVERBS_CMD_CREATE_QP => {
            let req: CreateQpRequest = copy_from_user(user_memory)?;
            let qp_num = uverbs_create_qp(minor, &req).map_err(|_| Errno::EINVAL)?;
            copy_to_user(user_memory, &qp_num)
        }
        UVERBS_CMD_MODIFY_QP => {
            let req: ModifyQpRequest = copy_from_user(user_memory)?;
            uverbs_modify_qp(minor, req).map_err(|_| Errno::EINVAL)?;
            Ok(0)
        }
        UVERBS_CMD_POLL_CQ => {
            let req: PollCqRequest = copy_from_user(user_memory)?;
            let wc_len = user_memory.out_size as usize / size_of::<ibv_wc>();
            let supported_len = wc_len.min(UVERBS_MAX_USER_WC_REQ);
            let mut wc_buf = Vec::with_capacity(supported_len);
            let wc_count = uverbs_poll_cq(minor, req.cq_num, &mut wc_buf).map_err(|_| Errno::EINVAL)?;
            copy_slice_to_user(user_memory, &wc_buf[..wc_count])
        }
        // for now we just check the ibv_send_wr struct, not the internal pointers it points to which
        // needs to be done to prevent security issues !
        UVERBS_CMD_POST_SEND => {
            let buf = copy_vec_from_user(user_memory)?;
            let (req,_): (PostSendRequest, usize)  = bincode::decode_from_slice(&buf, bincode::config::standard()).map_err(|_| Errno::EINVAL)?;
            uverbs_post_send(minor, &req).map_err(|_| Errno::EINVAL);
            Ok(0)
        }
        // same as above
        UVERBS_CMD_POST_RECV => {
            let buf = copy_vec_from_user(user_memory)?;
            let (req,_): (PostReceiveRequest, usize) = bincode::decode_from_slice(&buf, bincode::config::standard()).map_err(|_| Errno::EINVAL)?;
            uverbs_post_recv(minor, &req).map_err(|_| Errno::EINVAL);
            Ok(0)
        }
        UVERBS_CMD_DESTROY_CQ => {
            let cq_num: u32 = copy_from_user(user_memory)?;
            uverbs_destroy(minor, ConnectX3Nic::destroy_cq, cq_num).map_err(|_| Errno::EINVAL)?;
            Ok(0)
        }
        UVERBS_CMD_DESTROY_QP => {
            let qp_num: u32 = copy_from_user(user_memory)?;
            uverbs_destroy(minor, ConnectX3Nic::destroy_qp, qp_num).map_err(|_| Errno::EINVAL)?;
            Ok(0)
        }
        UVERBS_CMD_DEREGISTER_MR => {
            let mr_index: u32 = copy_from_user(user_memory)?;
            uverbs_destroy(minor, ConnectX3Nic::destroy_mr, mr_index).map_err(|_| Errno::EINVAL)?;
            Ok(0)
        }
        _ => Err(Errno::ENOCMD),
    }
}

#[inline]
fn copy_from_user<T: Copy>(user_memory: UserMemory) -> Result<T, Errno> {
    assert_ne!(user_memory.in_address, 0);

    let size = size_of::<T>();
    assert!(user_memory.in_size as usize >= size_of::<T>());

    let process = process_manager().read().current_process();
    let mut req = MaybeUninit::<T>::uninit();
    unsafe { process.virtual_address_space.copy_bytes_from_user(req.as_mut_ptr() as *mut u8, VirtAddr::new(user_memory.in_address), size) }
        .map_err(|e| Errno::EFAULT)?;
    Ok(unsafe { req.assume_init() })
}

#[inline]
fn copy_vec_from_user(user_memory: UserMemory) -> Result<Vec<u8>, Errno> {
    assert_ne!(user_memory.in_address, 0);
    let process = process_manager().read().current_process();
    let mut buf = vec![0u8; user_memory.in_size as usize];
    unsafe { process.virtual_address_space.copy_bytes_from_user(buf.as_mut_ptr(), VirtAddr::new(user_memory.in_address), buf.len()) }
        .map_err(|e| Errno::EFAULT)?;
    Ok(buf)
}

#[inline]
fn copy_to_user<T: Copy>(user_memory: UserMemory, resp: &T) -> SyscallResult {
    assert_ne!(user_memory.out_address, 0);

    let size = size_of::<T>();
    assert!(user_memory.out_size as usize >= size_of::<T>());

    let process = process_manager().read().current_process();
    unsafe { process.virtual_address_space.copy_bytes_to_user(VirtAddr::new(user_memory.out_address), resp as *const T as *const _, size) }
        .map_err(|e| Errno::EFAULT)
        .map(|()| 0)
}

#[inline]
fn copy_slice_to_user<T: Copy>(user_memory: UserMemory, resp: &[T]) -> SyscallResult {
    assert_ne!(user_memory.out_address, 0);

    let size = resp.len() * size_of::<T>();
    assert!(user_memory.out_size as usize >= size);

    let process = process_manager().read().current_process();
    unsafe { process.virtual_address_space.copy_bytes_to_user(VirtAddr::new(user_memory.out_address), resp.as_ptr() as *const u8, size) }
        .map_err(|e| Errno::EFAULT)
        .map(|()| resp.len())
}
