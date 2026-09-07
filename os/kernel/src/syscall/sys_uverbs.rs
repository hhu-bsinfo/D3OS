use rdma::uverbs_uapi::{UserSlice, UverbsCmd};
use crate::infiniband::uverbs::uverbs_ctl;
use syscall::return_vals;
use syscall::return_vals::Errno;

pub fn sys_uverbs_ctl(
    device_handle: usize,
    cmd: u64,
    in_address: u64,
    in_size: usize,
    out_address: u64,
    out_size: usize,
) -> isize {
    let user_in = UserSlice::new(in_address, in_size);
    let user_out = UserSlice::new(out_address, out_size);
    let Some(cmd) = UverbsCmd::from_repr(cmd) else {
        return Errno::EINVAL as isize
    };

    return_vals::convert_syscall_result_to_ret_code(
        uverbs_ctl(device_handle, cmd, user_in, user_out))
}
