use crate::infiniband::uverbs::uverbs_ctl;
use syscall::return_vals;

pub fn sys_uverbs_ctl(minor: usize, cmd: usize, arg: usize) -> isize {
    return_vals::convert_syscall_result_to_ret_code(
        uverbs_ctl(minor, cmd, arg))
}