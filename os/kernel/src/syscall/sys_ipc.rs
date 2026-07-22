use core::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use concurrent::ipc::{IpcMailboxDistributer, IpcMessage2, IpcQueue, IpcSmallMessageRingbuffer};
use spin::{Mutex, Once};
use syscall::return_vals::{self, SyscallResult};
use uuid::Uuid;

use crate::{consts, process::core_local_storage::scheduler, sync::wait_queue::WaitQueue};

static WAIT_QUEUE: WaitQueue = WaitQueue::new();

pub extern "sysv64" fn sys_ipc_send(pid1: u64, pid2: u64, tid: usize, msg_ptr: *const u8, msg_len: usize) -> isize {
    
    let pid = Uuid::from_u64_pair(pid1, pid2);

    if let Some(thread) = scheduler().thread(tid) {
        let vas = &thread.process().virtual_address_space;
        if let Some(ipc_queue_ptr) = vas.translate_ptr_mut(consts::USER_SPACE_MAILBOX_START as *mut IpcQueue) {
            if let Some(ipc_queue) = unsafe { ipc_queue_ptr.as_mut() } {
            
                let message = unsafe { core::slice::from_raw_parts(msg_ptr, msg_len) };
                let res = ipc_queue.send(message);

                if res {
                    return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(message.len()));
                }   
            }
        }        
    }

    return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(return_vals::Errno::EEXIST))
}

pub extern "sysv64" fn sys_ipc_send_small(pid1: u64, pid2: u64, tid: usize, message: u64) -> isize {

    let pid = Uuid::from_u64_pair(pid1, pid2);

    // TODO find process + thread globally (using name service etc.)
    if let Some(thread) = scheduler().thread(tid) {
        let vas = &thread.process().virtual_address_space;
        if let Some(mailbox_ptr) = vas.translate_ptr(consts::USER_SPACE_MAILBOX_START as *const &mut IpcSmallMessageRingbuffer) {
            let small_msg_ringbuffer_ref = unsafe { mailbox_ptr.read() };
            if let Some(small_msg_ringbuffer_ptr) = vas.translate_ptr_mut(core::ptr::from_mut(small_msg_ringbuffer_ref)) {
                if let Some(small_msg_ringbuffer) = unsafe { small_msg_ringbuffer_ptr.as_mut() } {
                
                    let res = small_msg_ringbuffer.send(message);

                    let space = small_msg_ringbuffer.space_left();
                    let messages = small_msg_ringbuffer.messages_left();

                    if res.is_ok() {
                        return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(message as usize));
                    }   
                }
            }
        }        
    }
    return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(return_vals::Errno::ENOTEMPTY))
}

pub extern "sysv64" fn sys_ipc_receive_blocking(timeout_ms: usize) -> isize {

    /*
    let handles_ptr_ptr = consts::USER_SPACE_MAILBOX_START as *mut *const IpcQueueHandles;
    let handles_ptr = unsafe { *handles_ptr_ptr };
    let handles = unsafe { handles_ptr.as_ref().unwrap() };

    let num_messages_atomic = &handles.num_messages;
    let num_messages = num_messages_atomic.load(Ordering::SeqCst);
    if num_messages > 0 {
        return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(num_messages as usize));
    }
    let current_time = timer().systime_ms();
    let timeout_time = current_time + timeout_ms;
    WAIT_QUEUE.wait(|| num_messages_atomic.load(Ordering::SeqCst) > 0 || timer().systime_ms() >= timeout_time, "waiting for ipc receive");
    if num_messages_atomic.load(Ordering::SeqCst) > 0 {
        return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(num_messages_atomic.load(Ordering::SeqCst) as usize))
    } else {
        return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(return_vals::Errno::EEXIST))
    }
     */
    return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(return_vals::Errno::EEXIST))
}