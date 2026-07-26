use super::uverbs_cmd::*;
use crate::device::mlx4::{device_in_range, ConnectX3Nic};
use crate::process_manager;
use alloc::vec;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::slice::from_raw_parts_mut;
use log::{debug, error};
use rdma::uverbs_uapi::{PostReceiveRequest, QueryPortRequest, UserMemory, UVERBS_MAX_USER_WC_REQ};
use rdma::{ibv_device, ibv_wc, uverbs_uapi::{
    CreateCqRequest, CreateMrRequest, CreateQpRequest, ModifyQpRequest,
    PollCqRequest, PostSendRequest, UverbsCmd,
}};
use syscall::return_vals::{Errno, SyscallResult};
use x86_64::VirtAddr;

/// user_in is a pointer to the parameters provided by the user
/// user_out points to a user buffer for return values
pub fn uverbs_ctl(device_handle: usize, cmd: UverbsCmd, user_memory_addr: *const UserMemory) -> SyscallResult {
    debug!("Uverbs device:{}, cmd:{:?}", device_handle, cmd);

    let requires_device_handle = match cmd {
        UverbsCmd::QueryDevices => false,
        _ => true
    };

    if requires_device_handle && !device_in_range(device_handle) {
        error!("{:?} requires a device handle, but {} is out of range", cmd, device_handle);
        return Err(Errno::EINVAL);
    }

    if !requires_device_handle && device_handle != 0 {
        error!("{:?} requires no device handle, but got {}", cmd, device_handle);
        return Err(Errno::EINVAL);
    }

    let process = process_manager().read().current_process();
    let mut user_memory = MaybeUninit::<UserMemory>::uninit();
    unsafe { process.virtual_address_space.copy_bytes_from_user(user_memory.as_mut_ptr() as *mut u8, VirtAddr::from_ptr(user_memory_addr), size_of::<UserMemory>()) }.map_err(|_| Errno::EFAULT)?;
    let user_memory: UserMemory = unsafe { user_memory.assume_init() };
    debug!("Uverbs user memory: {:?}", user_memory);


    match cmd {
        UverbsCmd::QueryDevices => {
            let devices = uverbs_query_devices(user_memory.out_size as usize / size_of::<ibv_device>());
            copy_slice_to_user(user_memory, &devices)
        }
        UverbsCmd::QueryDevice => {
            let dev_attr = uverbs_query_device(device_handle).map_err(log_error_and_invalid)?;
            copy_to_user(user_memory, &dev_attr)
        }
        UverbsCmd::QueryPort => {
            let mut req: QueryPortRequest = copy_from_user(user_memory)?;
            let port_attr = uverbs_query_port(device_handle, req.port_num).map_err(log_error_and_invalid)?;
            copy_to_user(user_memory, &port_attr)
        }
        UverbsCmd::RegMr => {
            let mut req: CreateMrRequest = copy_from_user(user_memory)?;
            // todo: use a custom type like UserSlice instead of slice
            let user_slice = unsafe { from_raw_parts_mut(req.data_ptr, req.len) };
            let resp = uverbs_register_mem_region(device_handle, req.ibv_access_flags, user_slice).map_err(log_error_and_invalid)?;
            copy_to_user(user_memory, &resp)
        }
        UverbsCmd::CreateCq => {
            let req: CreateCqRequest = copy_from_user(user_memory)?;
            let resp = uverbs_create_cq(device_handle, &req).map_err(log_error_and_invalid)?;
            copy_to_user(user_memory, &resp)
        }
        UverbsCmd::CreateQp => {
            let req: CreateQpRequest = copy_from_user(user_memory)?;
            let qp_num = uverbs_create_qp(device_handle, &req).map_err(log_error_and_invalid)?;
            copy_to_user(user_memory, &qp_num)
        }
        UverbsCmd::ModifyQp => {
            let req: ModifyQpRequest = copy_from_user(user_memory)?;
            uverbs_modify_qp(device_handle, req).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::PollCq => {
            let req: PollCqRequest = copy_from_user(user_memory)?;
            let wc_len = user_memory.out_size as usize / size_of::<ibv_wc>();
            let supported_len = wc_len.min(UVERBS_MAX_USER_WC_REQ);
            let mut wc_buf = Vec::with_capacity(supported_len);
            let wc_count = uverbs_poll_cq(device_handle, req.cq_num, &mut wc_buf).map_err(log_error_and_invalid)?;
            copy_slice_to_user(user_memory, &wc_buf[..wc_count])
        }
        // for now we just check the ibv_send_wr struct, not the internal pointers it points to which
        // needs to be done to prevent security issues !
        UverbsCmd::OpPostSend => {
            let buf = copy_vec_from_user(user_memory)?;
            let (req,_): (PostSendRequest, usize)  = bincode::decode_from_slice(&buf, bincode::config::standard()).map_err(|_| Errno::EINVAL)?;
            uverbs_post_send(device_handle, &req).map_err(log_error_and_invalid);
            Ok(0)
        }
        // same as above
        UverbsCmd::OpPostRecv => {
            let buf = copy_vec_from_user(user_memory)?;
            let (req,_): (PostReceiveRequest, usize) = bincode::decode_from_slice(&buf, bincode::config::standard()).map_err(|_| Errno::EINVAL)?;
            uverbs_post_recv(device_handle, &req).map_err(log_error_and_invalid);
            Ok(0)
        }
        UverbsCmd::DestroyCq => {
            let cq_num: u32 = copy_from_user(user_memory)?;
            uverbs_destroy(device_handle, ConnectX3Nic::destroy_cq, cq_num).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::DestroyQp => {
            let qp_num: u32 = copy_from_user(user_memory)?;
            uverbs_destroy(device_handle, ConnectX3Nic::destroy_qp, qp_num).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::DeregMr => {
            let mr_index: u32 = copy_from_user(user_memory)?;
            uverbs_destroy(device_handle, ConnectX3Nic::destroy_mr, mr_index).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::QueryQp => todo!("QueryQp"),
        UverbsCmd::SetMrSize => todo!("SetMrSize"),
    }
}

fn log_error_and_invalid(msg: &str) -> Errno {
    error!("{}", msg);
    Errno::EINVAL
}

#[inline]
fn copy_from_user<T: Copy>(user_memory: UserMemory) -> Result<T, Errno> {
    assert_ne!(user_memory.in_address, 0);

    let size = size_of::<T>();
    assert!(user_memory.in_size as usize >= size_of::<T>());

    let process = process_manager().read().current_process();
    let mut req = MaybeUninit::<T>::uninit();
    unsafe { process.virtual_address_space.copy_bytes_from_user(req.as_mut_ptr() as *mut u8, VirtAddr::new(user_memory.in_address), size) }
        .map_err(|_| Errno::EFAULT)?;
    Ok(unsafe { req.assume_init() })
}

#[inline]
fn copy_vec_from_user(user_memory: UserMemory) -> Result<Vec<u8>, Errno> {
    assert_ne!(user_memory.in_address, 0);
    let process = process_manager().read().current_process();
    let mut buf = vec![0u8; user_memory.in_size as usize];
    unsafe { process.virtual_address_space.copy_bytes_from_user(buf.as_mut_ptr(), VirtAddr::new(user_memory.in_address), buf.len()) }
        .map_err(|_| Errno::EFAULT)?;
    Ok(buf)
}

#[inline]
fn copy_to_user<T: Copy>(user_memory: UserMemory, resp: &T) -> SyscallResult {
    assert_ne!(user_memory.out_address, 0);

    let size = size_of::<T>();
    assert!(user_memory.out_size as usize >= size_of::<T>());

    let process = process_manager().read().current_process();
    unsafe { process.virtual_address_space.copy_bytes_to_user(VirtAddr::new(user_memory.out_address), resp as *const T as *const _, size) }
        .map_err(|_| Errno::EFAULT)
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
