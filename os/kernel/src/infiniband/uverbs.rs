use super::uverbs_cmd::*;
use crate::device::mlx4::{device_in_range, ConnectX3Nic};
use crate::process_manager;
use core::mem::MaybeUninit;
use core::slice::from_raw_parts_mut;
use log::error;
use rdma::uverbs_uapi::{QueryPortRequest, UserSlice};
use rdma::{ibv_device, uverbs_uapi::{
    CreateCqRequest, CreateMrRequest, CreateQpRequest, ModifyQpRequest, UverbsCmd,
}};
use syscall::return_vals::{Errno, SyscallResult};
use x86_64::VirtAddr;

/// user_in describes the parameters provided by the user
/// user_out describes a user buffer for return values
pub fn uverbs_ctl(device_handle: usize, cmd: UverbsCmd, user_in: UserSlice, user_out: UserSlice) -> SyscallResult {
    //debug!("Uverbs device:{}, cmd:{:?}, in:{:?}, out:{:?}", device_handle, cmd, user_in, user_out);

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

    let result = dispatch(device_handle, cmd, user_in, user_out);

    // Drain the card's event queue right after the verb that may have provoked it. The card
    // reports a port going down or a queue pair failing as an event, and those used to be
    // consumed only from `ibv_poll_cq`, so they surfaced at some arbitrary later moment with no
    // way to tell which operation caused them. When the queue is empty this is a single read of
    // the ring.
    if requires_device_handle {
        uverbs_drain_events(device_handle);
    }

    result
}

fn dispatch(device_handle: usize, cmd: UverbsCmd, user_in: UserSlice, user_out: UserSlice) -> SyscallResult {
    match cmd {
        UverbsCmd::QueryDevices => {
            let devices = uverbs_query_devices(user_out.capacity::<ibv_device>());
            copy_slice_to_user(user_out, &devices)
        }
        UverbsCmd::QueryDevice => {
            let dev_attr = uverbs_query_device(device_handle).map_err(log_error_and_invalid)?;
            copy_to_user(user_out, &dev_attr)
        }
        UverbsCmd::QueryPort => {
            let mut req: QueryPortRequest = copy_from_user(user_in)?;
            let port_attr = uverbs_query_port(device_handle, req.port_num).map_err(log_error_and_invalid)?;
            copy_to_user(user_out, &port_attr)
        }
        UverbsCmd::RegMr => {
            let mut req: CreateMrRequest = copy_from_user(user_in)?;
            // todo: use a custom type like UserSlice instead of slice
            let user_slice = unsafe { from_raw_parts_mut(req.data_ptr, req.len) };
            let resp = uverbs_register_mem_region(device_handle, req.ibv_access_flags, user_slice).map_err(log_error_and_invalid)?;
            copy_to_user(user_out, &resp)
        }
        UverbsCmd::CreateCq => {
            let req: CreateCqRequest = copy_from_user(user_in)?;
            let resp = uverbs_create_cq(device_handle, &req).map_err(log_error_and_invalid)?;
            copy_to_user(user_out, &resp)
        }
        UverbsCmd::CreateQp => {
            let req: CreateQpRequest = copy_from_user(user_in)?;
            let resp = uverbs_create_qp(device_handle, &req).map_err(log_error_and_invalid)?;
            copy_to_user(user_out, &resp)
        }
        UverbsCmd::ModifyQp => {
            let req: ModifyQpRequest = copy_from_user(user_in)?;
            uverbs_modify_qp(device_handle, req).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::DrainEvents => {
            uverbs_drain_events(device_handle);
            Ok(0)
        }
        UverbsCmd::DestroyCq => {
            let cq_num: u32 = copy_from_user(user_in)?;
            uverbs_destroy(device_handle, ConnectX3Nic::destroy_cq, cq_num).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::DestroyQp => {
            let qp_num: u32 = copy_from_user(user_in)?;
            uverbs_destroy(device_handle, ConnectX3Nic::destroy_qp, qp_num).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::DeregMr => {
            let mr_index: u32 = copy_from_user(user_in)?;
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
fn copy_from_user<T: Copy>(user_in: UserSlice) -> Result<T, Errno> {
    assert_ne!(user_in.address, 0);

    let size = size_of::<T>();
    assert!(user_in.size >= size);

    let process = process_manager().read().current_process();
    let mut req = MaybeUninit::<T>::uninit();
    unsafe { process.virtual_address_space.copy_bytes_from_user(req.as_mut_ptr() as *mut u8, VirtAddr::new(user_in.address), size) }
        .map_err(|_| {
            error!("copy_from_user failed: address={:#x}, size={} (requested {})", user_in.address, size, user_in.size);
            Errno::EFAULT
        })?;
    Ok(unsafe { req.assume_init() })
}

#[inline]
fn copy_to_user<T: Copy>(user_out: UserSlice, resp: &T) -> SyscallResult {
    assert_ne!(user_out.address, 0);

    let size = size_of::<T>();
    assert!(user_out.size >= size);

    let process = process_manager().read().current_process();
    unsafe { process.virtual_address_space.copy_bytes_to_user(VirtAddr::new(user_out.address), resp as *const T as *const _, size) }
        .map_err(|_| {
            error!("copy_to_user failed: address={:#x}, size={} (buffer {})", user_out.address, size, user_out.size);
            Errno::EFAULT
        })
        .map(|()| 0)
}

#[inline]
fn copy_slice_to_user<T: Copy>(user_out: UserSlice, resp: &[T]) -> SyscallResult {
    assert_ne!(user_out.address, 0);

    let size = size_of_val(resp);
    assert!(user_out.size >= size);

    let process = process_manager().read().current_process();
    unsafe { process.virtual_address_space.copy_bytes_to_user(VirtAddr::new(user_out.address), resp.as_ptr() as *const u8, size) }
        .map_err(|_| {
            error!("copy_slice_to_user failed: address={:#x}, size={} (buffer {}, {} elements)", user_out.address, size, user_out.size, resp.len());
            Errno::EFAULT
        })
        .map(|()| resp.len())
}
