use core::str;

use alloc::{format, slice};
use concurrent::ipc::{IpcQueue, IpcSyscallParams};
use naming::shared_types::OpenOptions;
use syscall::return_vals::{self, SyscallResult};

use crate::{consts, naming::api, process::core_local_storage::scheduler, sync::wait_queue::WaitQueue, timer};

static WAIT_QUEUE: WaitQueue = WaitQueue::new();

pub extern "sysv64" fn sys_ipc_register(name_ptr: *const u8, name_len: usize) -> isize {
    
    let bytes = unsafe { slice::from_raw_parts(name_ptr, name_len) };
    let name = str::from_utf8(bytes).unwrap();

    // create a file in /svc/<name> to register this process' ipc endpoint
    match api::open(&format!("/svc/{}", name), OpenOptions::CREATE) {
        Ok(fh) => {
            api::close(fh);
            return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(0))
        }
        Err(err) => return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(err))
    }
}

pub extern "sysv64" fn sys_ipc_send(params_ptr: *const IpcSyscallParams) -> isize {

    let params = unsafe { &*params_ptr };
    
    let service_name = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(params.service_name_ptr, params.service_name_len)) };
    
    if let Ok(fh) = api::open(&format!("/svc/{}", service_name), OpenOptions::READONLY) {

        // read usize using u8 buffer
        let mut buf = [0_u8; 8];
        let _res = api::read(fh, buf.as_mut_slice());
        api::close(fh);
        let ipc_queue_addr = usize::from_le_bytes(buf);

        let ipc_queue_ptr = ipc_queue_addr as *const IpcQueue;
        if let Some(ipc_queue) = unsafe { ipc_queue_ptr.as_ref() } {
            
            let message = unsafe { core::slice::from_raw_parts(params.data_ptr, params.data_len) };
            let tag = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(params.tag_ptr, params.tag_len)) };
            let res = ipc_queue.send(tag, message, &scheduler().current_thread().process().id());

            if res {
                return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(message.len()));
            }   
        }
    }

    return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(return_vals::Errno::EEXIST))
}

pub extern "sysv64" fn sys_ipc_receive_blocking(timeout_ms: usize) -> isize {

    let ipc_queue_ptr = consts::USER_SPACE_MAILBOX_START as *mut IpcQueue;
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
            return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(num_messages))
        } else {
            return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(return_vals::Errno::EEXIST))
        }
    }
    return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(return_vals::Errno::EEXIST))
}