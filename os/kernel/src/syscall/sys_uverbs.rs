use syscall::return_vals::Errno;

#[cfg(feature = "infiniband_mlx4")]
pub fn sys_uverbs_ctl(
    device_handle: usize,
    cmd: u64,
    in_address: u64,
    in_size: usize,
    out_address: u64,
    out_size: usize,
) -> isize {
    use crate::device::infiniband::uverbs::uverbs_ctl;
    use rdma::uverbs_uapi::{UserSlice, UverbsCmd};
    use syscall::return_vals;

    let user_in = UserSlice::new(in_address, in_size);
    let user_out = UserSlice::new(out_address, out_size);
    let Some(cmd) = UverbsCmd::from_repr(cmd) else {
        return Errno::EINVAL as isize;
    };

    return_vals::convert_syscall_result_to_ret_code(
        uverbs_ctl(device_handle, cmd, user_in, user_out))
}

// TODO: implement a more general way to dispatch different device types
/// Without an InfiniBand driver the syscall slot stays, so the syscall numbering is unchanged.
#[cfg(not(feature = "infiniband_mlx4"))]
pub fn sys_uverbs_ctl(_device_handle: usize, _cmd: u64, _in_address: u64, _in_size: usize, _out_address: u64, _out_size: usize) -> isize {
    Errno::ENOTSUP as isize
}
