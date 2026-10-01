use super::uverbs_cmd::*;
use crate::device::infiniband::mlx4::{Mlx4Device, device_in_range};
use crate::process::process::Process;
use crate::process::core_local_storage::scheduler;
use core::mem::{MaybeUninit, offset_of};
use log::error;
use rdma::uverbs_uapi::{
    AllocPdRequest, AllocPdResponse, CreateCqRequest, CreateMrRequest, CreateQpRequest, DeallocPdRequest, DestroyRequest, ModifyQpRequest,
    QueryPortRequest, UserSlice, UverbsCmd,
};
use rdma::{DeviceHandle, Mtu, QueuePairState, QueuePairType};
use syscall::return_vals::{Errno, SyscallResult};
use x86_64::VirtAddr;
use zerocopy::FromBytes;

/// user_in describes the parameters provided by the user
/// user_out describes a user buffer for return values
pub fn uverbs_ctl(device_handle: usize, cmd: UverbsCmd, user_in: UserSlice, user_out: UserSlice) -> SyscallResult {
    //debug!("Uverbs device:{}, cmd:{:?}, in:{:?}, out:{:?}", device_handle, cmd, user_in, user_out);

    let requires_device_handle = match cmd {
        UverbsCmd::QueryDevices => false,
        _ => true,
    };

    if requires_device_handle && !device_in_range(device_handle) {
        error!("{:?} requires a device handle, but {} is out of range", cmd, device_handle);
        return Err(Errno::EINVAL);
    }

    if !requires_device_handle && device_handle != 0 {
        error!("{:?} requires no device handle, but got {}", cmd, device_handle);
        return Err(Errno::EINVAL);
    }

    // Resolved once and without the process manager lock, so that no device lock below ever
    // nests it.
    let process = scheduler().current_thread().process();
    dispatch(&process, device_handle, cmd, user_in, user_out)
}

fn dispatch(process: &Process, device_handle: usize, cmd: UverbsCmd, user_in: UserSlice, user_out: UserSlice) -> SyscallResult {
    match cmd {
        UverbsCmd::QueryDevices => {
            let devices = uverbs_query_devices(user_out.capacity::<DeviceHandle>());
            copy_slice_to_user(process, user_out, &devices)
        }
        UverbsCmd::QueryDevice => {
            let dev_attr = uverbs_query_device(device_handle).map_err(log_error_and_invalid)?;
            copy_to_user(process, user_out, &dev_attr)
        }
        UverbsCmd::QueryPort => {
            let req: QueryPortRequest = copy_from_user(process, user_in)?;
            let port_attr = uverbs_query_port(device_handle, req.port_num).map_err(log_error_and_invalid)?;
            copy_to_user(process, user_out, &port_attr)
        }
        UverbsCmd::RegMr => {
            let req: CreateMrRequest = copy_from_user(process, user_in)?;
            let resp = uverbs_register_mem_region(device_handle, process, &req).map_err(log_error_and_invalid)?;
            copy_to_user(process, user_out, &resp)
        }
        UverbsCmd::CreateCq => {
            let req: CreateCqRequest = copy_from_user(process, user_in)?;
            let resp = uverbs_create_cq(device_handle, process, &req).map_err(log_error_and_invalid)?;
            copy_to_user(process, user_out, &resp)
        }
        UverbsCmd::CreateQp => {
            let req: CreateQpRequest = copy_from_user(process, user_in)?;
            let resp = uverbs_create_qp(device_handle, process, &req).map_err(log_error_and_invalid)?;
            copy_to_user(process, user_out, &resp)
        }
        UverbsCmd::ModifyQp => {
            let req: ModifyQpRequest = copy_from_user(process, user_in)?;
            uverbs_modify_qp(device_handle, process, req).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::DestroyCq => {
            let req: DestroyRequest = copy_from_user(process, user_in)?;
            uverbs_destroy(device_handle, process, Mlx4Device::destroy_cq, req).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::DestroyQp => {
            let req: DestroyRequest = copy_from_user(process, user_in)?;
            uverbs_destroy(device_handle, process, Mlx4Device::destroy_qp, req).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::DeregMr => {
            let req: DestroyRequest = copy_from_user(process, user_in)?;
            uverbs_destroy(device_handle, process, Mlx4Device::destroy_mr, req).map_err(log_error_and_invalid)?;
            Ok(0)
        }
        UverbsCmd::QueryQp | UverbsCmd::SetMrSize => {
            error!("{:?} is not supported", cmd);
            Err(Errno::ENOTSUP)
        }
        UverbsCmd::OpenDevice => {
            let resp = uverbs_open_device(device_handle, process).map_err(log_error_and_invalid)?;
            copy_to_user(process, user_out, &resp)
        }
        UverbsCmd::AllocPd => {
            let req: AllocPdRequest = copy_from_user(process, user_in)?;
            let resp: AllocPdResponse = uverbs_alloc_pd(device_handle, process, req).map_err(log_error_and_invalid)?;
            copy_to_user(process, user_out, &resp)
        }
        UverbsCmd::DeallocPd => {
            let req: DeallocPdRequest = copy_from_user(process, user_in)?;
            uverbs_dealloc_pd(device_handle, process, req).map_err(log_error_and_invalid)?;
            Ok(0)
        }
    }
}

fn log_error_and_invalid(msg: &str) -> Errno {
    error!("{}", msg);
    Errno::EINVAL
}

#[inline]
fn copy_from_user<T: Copy>(process: &Process, user_in: UserSlice) -> Result<T, Errno> {
    let size = size_of::<T>();
    if user_in.address == 0 || user_in.size < size {
        return Err(Errno::EINVAL);
    }

    let mut req = MaybeUninit::<T>::uninit();
    unsafe {
        process
            .virtual_address_space
            .copy_bytes_from_user(req.as_mut_ptr() as *mut u8, VirtAddr::new(user_in.address), size)
    }
    .map_err(|_| {
        error!(
            "copy_from_user failed: address={:#x}, size={} (requested {})",
            user_in.address, size, user_in.size
        );
        Errno::EFAULT
    })?;
    Ok(unsafe { req.assume_init() })
}

#[inline]
fn copy_to_user<T: Copy>(process: &Process, user_out: UserSlice, resp: &T) -> SyscallResult {
    let size = size_of::<T>();
    if user_out.address == 0 || user_out.size < size {
        return Err(Errno::EINVAL);
    }

    unsafe {
        process
            .virtual_address_space
            .copy_bytes_to_user(VirtAddr::new(user_out.address), resp as *const T as *const _, size)
    }
    .map_err(|_| {
        error!("copy_to_user failed: address={:#x}, size={} (buffer {})", user_out.address, size, user_out.size);
        Errno::EFAULT
    })
    .map(|()| 0)
}

#[inline]
fn copy_slice_to_user<T: Copy>(process: &Process, user_out: UserSlice, resp: &[T]) -> SyscallResult {
    let size = size_of_val(resp);
    if user_out.address == 0 || user_out.size < size {
        return Err(Errno::EINVAL);
    }

    unsafe {
        process
            .virtual_address_space
            .copy_bytes_to_user(VirtAddr::new(user_out.address), resp.as_ptr() as *const u8, size)
    }
    .map_err(|_| {
        error!(
            "copy_slice_to_user failed: address={:#x}, size={} (buffer {}, {} elements)",
            user_out.address,
            size,
            user_out.size,
            resp.len()
        );
        Errno::EFAULT
    })
    .map(|()| resp.len())
}
