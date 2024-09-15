/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: consts.rs                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Consts and types for syscall return values.                     ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 15.09.2024, HHU                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use num_enum::{FromPrimitive, IntoPrimitive};

#[derive(Debug, Copy, Clone, Eq, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(isize)]
pub enum Errno {
    #[num_enum(default)]
    EUNKN = -1, // Unknown error
    ENOENT = -2,     // No such file or directory
	ENOHANDLES = -3,    // No more free handles
    EACCES = -13,    // Permission denied
    EEXIST = -17,    // File/directory exists
    ENOTDIR = -20,   // Not a directory
    EINVAL = -22,    // Invalid argument
    EINVALH   = -23,    // Invalid handle
    ENOTEMPTY = -90, // Directory not empty
}

bitflags! {
    /// Description: Option flags for opening objects
    pub struct OpenOptions: u32 {
        const READONLY  = 0x01;
        const READWRITE = 0x02;
        const CREATE    = 0x04;
    }
}

/// Enumeration of possible methods to seek within an I/O object.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SeekOrigin {
    Start,
    End,
    Current,
}

pub type SyscallResult = Result<usize, Errno>;

pub fn convert_ret_code_to_syscall_result(ret_code: isize) -> SyscallResult {
    if ret_code < 0 {
        Err(Errno::from(ret_code))
    } else {
        Ok(ret_code as usize)
    }
}

pub fn convert_syscall_result_to_ret_code(syscall_result: SyscallResult) -> isize {
    let ret_val: isize;
    match syscall_result {
        Ok(t) => ret_val = t as isize,
        Err(e) => ret_val = e.into(),
    }

    ret_val
}
