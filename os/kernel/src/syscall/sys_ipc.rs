use concurrent::ipc::IpcQueue;
use syscall::return_vals::{self, SyscallResult};
use uuid::Uuid;

use crate::{consts, process::core_local_storage::scheduler, sync::wait_queue::WaitQueue, timer};

static WAIT_QUEUE: WaitQueue = WaitQueue::new();

pub extern "sysv64" fn sys_ipc_send(pid1: u64, pid2: u64, tid: usize, msg_ptr: *const u8, msg_len: usize) -> isize {
    
    let pid = Uuid::from_u64_pair(pid1, pid2);

    if let Some(thread) = scheduler().thread(tid) {
        let vas = &thread.process().virtual_address_space;
        if let Some(ipc_queue_ptr) = vas.translate_ptr_mut(consts::USER_SPACE_MAILBOX_START as *mut IpcQueue) {
            if let Some(ipc_queue) = unsafe { ipc_queue_ptr.as_mut() } {
            
                let message = unsafe { core::slice::from_raw_parts(msg_ptr, msg_len) };
                let res = ipc_queue.send(message, pid, tid);

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