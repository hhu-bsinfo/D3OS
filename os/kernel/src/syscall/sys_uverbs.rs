use rdma::uverbs_uapi::{UserMemory, UverbsCmd};
use crate::infiniband::uverbs::uverbs_ctl;
use syscall::return_vals;

pub fn sys_uverbs_ctl(minor: usize, cmd: UverbsCmd, user_memory_ptr: *const UserMemory) -> isize {
    return_vals::convert_syscall_result_to_ret_code(
        uverbs_ctl(minor, cmd, user_memory_ptr))
}