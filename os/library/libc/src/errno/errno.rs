use core::ffi::c_int;

pub enum Errno {
    /// Parameter outside function domain
    EDOM = 1,
    /// Result outside of function range
    ERANGE = 2,
    /// Illegal byte sequence
    EILSEQ = 3,
    /// File being opened is a directory
    EISDIR = 4,
    /// File not found
    ENOENT = 5,
    /// File already exists
    EEXIST = 6,
    /// Invalid file descriptor
    EBADF = 7,
    /// Permission denied
    EACCES = 8,
}

#[unsafe(no_mangle)]
static mut error: c_int = 0;

pub fn set_errno(err: Errno) {
    unsafe { error = err as c_int; }
}