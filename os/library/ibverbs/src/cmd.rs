use core3::io;
use core3::io::{Error, ErrorKind};
use rdma::uverbs_uapi::{UserSlice, UverbsCmd};
use syscall::return_vals::Errno;

pub fn uverbs(
    device_fd: usize, cmd: UverbsCmd, user_in: UserSlice, user_out: UserSlice,
) -> io::Result<usize> {
    use syscall::{syscall, SystemCall::Uverb};

    syscall(Uverb, &[
        device_fd,
        cmd as u64 as usize,
        user_in.address as usize,
        user_in.size,
        user_out.address as usize,
        user_out.size,
    ]).map_err(uverbs_error)
}

fn uverbs_error(errno: Errno) -> Error {
    let msg = match errno {
        Errno::EUNKN => "uverbs: unknown error",
        Errno::ENOENT => "uverbs: no such file or directory",
        Errno::ENOHANDLES => "uverbs: no more free handles",
        Errno::EBADF => "uverbs: bad file descriptor",
        Errno::EACCES => "uverbs: permission denied",
        Errno::EEXIST => "uverbs: file/directory exists",
        Errno::ENOTDIR => "uverbs: not a directory",
        Errno::EINVAL => "uverbs: invalid argument",
        Errno::EINVALH => "uverbs: invalid handle",
        Errno::ENOTEMPTY => "uverbs: directory not empty",
        Errno::EBADSTR => "uverbs: bad string",
        Errno::EBUSY => "uverbs: device busy",
        Errno::ENOTSUP => "uverbs: operation not supported",
        Errno::ECONNRESET => "uverbs: connection reset by peer",
        Errno::ERDONLY => "uverbs: read-only file system",
        Errno::EAGAIN => "uverbs: resource unavailable",
        Errno::ESRCH => "uverbs: no such thread",
        Errno::EOF => "uverbs: end of file",
        Errno::EPIPE => "uverbs: broken pipe",
        Errno::ENOMEM => "uverbs: not enough space / cannot allocate memory",
        Errno::EISDIR => "uverbs: is a directory",
        Errno::EFAULT => "uverbs: fault occurred",
        Errno::ENOCMD => "uverbs: no such command",
    };
    Error::new(ErrorKind::Other, msg)
}

