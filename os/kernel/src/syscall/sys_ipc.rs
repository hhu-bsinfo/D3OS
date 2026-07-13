use core::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use concurrent::ipc::{IpcMailboxDistributer, IpcMessage, IpcSmallMessageRingbuffer};
use spin::{Mutex, Once};
use syscall::return_vals::{self, SyscallResult};
use uuid::Uuid;

use crate::{consts, process::core_local_storage::scheduler, sync::wait_queue::{self, WaitQueue}, timer};

static WAIT_QUEUE: WaitQueue = WaitQueue::new();

pub extern "sysv64" fn sys_ipc_send(pid1: u64, pid2: u64, tid: usize, message_addr: usize) -> isize {

    /*
    

    let pid = Uuid::from_u64_pair(pid1, pid2);

    // TODO find process + thread globally (using name service etc.)
    if let Some(thread) = scheduler().thread(tid) {
        if let Some(mailbox_addr_phys) = thread.process().virtual_address_space.get_phys(consts::USER_SPACE_MAILBOX_START as u64) {
            let handles_ptr_ptr = mailbox_addr_phys.as_u64() as *mut *const IpcQueueHandles;
            let handles_ptr_virt = unsafe { *handles_ptr_ptr };
            if let Some(handles_addr_phys) = thread.process().virtual_address_space.get_phys(handles_ptr_virt as u64) {
                let handles_ptr_phys = handles_addr_phys.as_u64() as *const IpcQueueHandles;
                let handles = unsafe { handles_ptr_phys.as_ref().unwrap() };
                let message = unsafe { *(message_addr as *const IpcMessage) };
                todo!("Messagequeue müsste eingemappt werden");
                // TODO die queue implementierung benutzt intern virtuelle addressen -> kann nicht von einem anderen process benutzt werden
                handles.sender.enqueue(message);
                handles.num_messages.fetch_add(1, Ordering::SeqCst);
                return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(0))
            }
        }
    }
    
     */

    return_vals::convert_syscall_result_to_ret_code(SyscallResult::Err(return_vals::Errno::EEXIST))
}

pub extern "sysv64" fn sys_ipc_send_small(pid1: u64, pid2: u64, tid: usize, message: u64) -> isize {

    let pid = Uuid::from_u64_pair(pid1, pid2);

    // TODO find process + thread globally (using name service etc.)
    if let Some(thread) = scheduler().thread(tid) {
        let test = {
            scheduler().current_thread().process().virtual_address_space.get_phys(consts::USER_SPACE_MAILBOX_START as u64).unwrap()
        };
        if let Some(mailbox_addr_phys) = thread.process().virtual_address_space.get_phys(consts::USER_SPACE_MAILBOX_START as u64) {
            let distributer = unsafe { (mailbox_addr_phys.as_u64() as *const IpcMailboxDistributer).read() };
            if let Some(small_msg_ringbuffer_addr_phys) = thread.process().virtual_address_space.get_phys(distributer.small_msg_ringbuffer as u64) {
                let small_msg_ringbuffer = unsafe { (small_msg_ringbuffer_addr_phys.as_u64() as *const IpcSmallMessageRingbuffer).read() };
                
                let res = small_msg_ringbuffer.send(message);

                if res.is_ok() {
                    return return_vals::convert_syscall_result_to_ret_code(SyscallResult::Ok(message as usize));
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