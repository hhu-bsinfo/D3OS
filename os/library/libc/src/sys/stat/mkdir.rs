use core::ffi::{c_char, c_int, c_uint};
use crate::errno::errno::{set_errno, Errno};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mkdir(_path: *const c_char, _mode: c_uint) -> c_int {
    set_errno(Errno::EACCES);
    -1
}