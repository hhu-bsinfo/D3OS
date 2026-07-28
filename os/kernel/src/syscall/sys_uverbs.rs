use rdma::uverbs_uapi::{UserSlice, UverbsCmd};
use crate::infiniband::uverbs::uverbs_ctl;
use syscall::return_vals;

pub fn sys_uverbs_ctl(
    minor: usize, cmd: UverbsCmd,
    in_address: u64, in_size: usize,
    out_address: u64, out_size: usize,
) -> isize {
    let user_in = UserSlice::new(in_address, in_size);
    let user_out = UserSlice::new(out_address, out_size);

    return_vals::convert_syscall_result_to_ret_code(
        uverbs_ctl(minor, cmd, user_in, user_out))
}
