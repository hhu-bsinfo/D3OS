use concurrent::ipc::{IpcQueue, IpcSyscallParams};
use syscall::return_vals::{self, SyscallResult};

use crate::{consts, process::core_local_storage::scheduler, sync::wait_queue::WaitQueue, timer};

static WAIT_QUEUE: WaitQueue = WaitQueue::new();

pub extern "sysv64" fn sys_ipc_send(params_ptr: *const IpcSyscallParams) -> isize {

    let params = unsafe { &*params_ptr };
    
    let pid = params.pid;
    let tid = params.tid;

    if let Some(thread) = scheduler().thread(tid) {
        let vas = &thread.process().virtual_address_space;
        if let Some(ipc_queue_ptr) = vas.translate_ptr_mut(consts::USER_SPACE_MAILBOX_START as *mut IpcQueue) {
            if let Some(ipc_queue) = unsafe { ipc_queue_ptr.as_mut() } {
            
                let message = unsafe { core::slice::from_raw_parts(params.data_ptr, params.data_len) };
                let tag = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(params.tag_ptr, params.tag_len)) };
                let res = ipc_queue.send(message, pid, tid, tag);

                if res {
                    return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(message.len()));
                }   
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