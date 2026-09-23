use core::str;

use alloc::{format, slice};
use ipc::{IpcEndpoints, IpcSyscallParams, MESSAGE_DATA_SIZE};
use ipc::queues::{IpcQueue, IpcQueueType};
use ipc::ipc_error::IpcError;
use log::error;
use naming::shared_types::OpenOptions;
use syscall::return_vals::{self, SyscallResult};
use uuid::Uuid;

use crate::naming::api as nameservice;
use crate::naming::servicefs::ServiceObject;
use crate::{consts, process::core_local_storage::scheduler, sync::wait_queue::WaitQueue, timer};

static WAIT_QUEUE: WaitQueue = WaitQueue::new();

pub extern "sysv64" fn sys_ipc_register(name_ptr: *const u8, name_len: usize) -> isize {
    
    let bytes = unsafe { slice::from_raw_parts(name_ptr, name_len) };
    let name = str::from_utf8(bytes).unwrap();

    // create a file in /svc/<name> to register this process' ipc endpoint
    match nameservice::open(&format!("/svc/{}", name), OpenOptions::CREATE) {
        Ok(fh) => {
            nameservice::close(fh);
            return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(0))
        }
        Err(err) => return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(err))
    }
}

pub extern "sysv64" fn sys_ipc_delete(name_ptr: *const u8, name_len: usize) -> isize {
    
    let bytes = unsafe { slice::from_raw_parts(name_ptr, name_len) };
    let name = str::from_utf8(bytes).unwrap();

    // remove the file in /svc/<name> to delete this process' ipc endpoint
    match nameservice::open(&format!("/svc/{}", name), OpenOptions::READWRITE) {
        Ok(fh) => {
            nameservice::delete(fh);
            return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(0))
        }
        Err(err) => return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(err))
    }
}

pub fn remove_endpoints_for_process(process_id: &Uuid) {
    if let Ok(svc_fh) = nameservice::open("/svc", OpenOptions::DIRECTORY) {
        // get all services
        if let Ok(list) = nameservice::list_dir(svc_fh) {
            for (_, name) in list {
                if let Ok(fh) = nameservice::open(&format!("/svc/{}", name), OpenOptions::READWRITE) {
                    let mut buf = [0_u8; core::mem::size_of::<ServiceObject>()];
                    nameservice::read(fh, &mut buf);
                    let service = unsafe { &(*(buf.as_ptr() as *const ServiceObject)).service };
                    if service.owning_pid == *process_id {
                        nameservice::delete(fh);
                    }
                }
            }
        }
    }
}

pub extern "sysv64" fn sys_ipc_send(params_ptr: *const IpcSyscallParams) -> isize {

    let params = unsafe { &*params_ptr };
            
    let message = unsafe { core::slice::from_raw_parts(params.data_ptr, params.data_len) };

    if message.len() > MESSAGE_DATA_SIZE {
        error!("ipc message is to large, aborting...");
        return IpcError::MsgTooLarge.into();
    }
    
    let service_name = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(params.service_name_ptr, params.service_name_len)) };
    
    if let Ok(fh) = nameservice::open(&format!("/svc/{}", service_name), OpenOptions::READONLY) {

        // read ServiceObject using u8 buffer
        let mut buf = [0_u8; core::mem::size_of::<ServiceObject>()];
        let _res = nameservice::read(fh, buf.as_mut_slice());
        nameservice::close(fh);
        let service = unsafe { &(*(buf.as_ptr() as *const ServiceObject)).service };

        let endpoints = unsafe { (service.ipc_queue_addr as *const IpcEndpoints).as_ref_unchecked() };
        if let Some(ipc_queue) = endpoints.get(service_name) {
            let name = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(params.msg_name_ptr, params.msg_name_len)) };
            let msg_type = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(params.msg_type_ptr, params.msg_type_len)) };
            let res = ipc_queue.send(name, msg_type, message, &scheduler().current_thread().process().id(), scheduler().current_thread().id());

            match res {
                Ok(_) => return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(message.len())),
                Err(err) => return err.into(),
            }
        }
    }

    IpcError::Generic.into()
}

pub extern "sysv64" fn sys_ipc_receive_blocking(timeout_ms: usize) -> isize {

    todo!("this needs to be updated to use the nameservice and the new endpoints struct");

    let ipc_queue_ptr = consts::USER_SPACE_MAILBOX_START as *mut IpcQueueType;
    if let Some(ipc_queue) = unsafe { ipc_queue_ptr.as_mut() } {

        let num_messages = ipc_queue.messages_left();
        if num_messages > 0 {
            return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(num_messages as usize));
        }
        let current_time = timer().systime_ms();
        let timeout_time = current_time + timeout_ms;
        WAIT_QUEUE.wait(|| ipc_queue.messages_left() > 0 || timer().systime_ms() >= timeout_time, "waiting for ipc receive");
        let num_messages = ipc_queue.messages_left();
        if num_messages > 0 {
            return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(num_messages));
        } else {
            return IpcError::Timeout.into();
        }
    }
    IpcError::Generic.into()
}