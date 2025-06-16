/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: consts.rs                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Consts and types for syscall return values.                     ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 27.12.2024, HHU                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use num_enum::{FromPrimitive, IntoPrimitive};

/// Description: error codes for syscalls
#[derive(Debug, Copy, Clone, Eq, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(isize)]
pub enum Errno {
    #[num_enum(default)]
    EUNKN      = -1,  // Unknown error
    ENOENT     = -2,  // No such file or directory
	ENOHANDLES = -3,  // No more free handles
    EBADF      = -4,  // Bad file descriptor for an operation
    EACCES     = -5,  // Permission denied
    EEXIST     = -6,  // File/directory exists
    ENOTDIR    = -7,  // Not a directory
    EINVAL     = -8,  // Invalid argument
    EINVALH    = -9,  // Invalid handle
    ENOTEMPTY  = -10, // Directory not empty
    EBADSTR    = -11, // Bad string
}


/// Description: Result type for syscalls
pub type SyscallResult = Result<usize, Errno>;

/// Description: convert a return code to a syscall result
pub fn convert_ret_code_to_syscall_result(ret_code: isize) -> SyscallResult {
    if ret_code < 0 {
        Err(Errno::from(ret_code))
    } else {
        Ok(ret_code as usize)
    }
}

/// Description: convert a syscall result to a return code
pub fn convert_syscall_result_to_ret_code(syscall_result: SyscallResult) -> isize {
    match syscall_result {
        Ok(t) => t as isize,
        Err(e) => e.into(),
    }
}
