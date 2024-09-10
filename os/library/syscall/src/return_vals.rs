/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: consts.rs                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Consts and types for syscall return values.                     ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 10.09.2024, HHU                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use num_enum::{FromPrimitive, IntoPrimitive};

#[derive(Debug, Copy, Clone, Eq, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(i64)]
pub enum Errno {
    #[num_enum(default)]
    EUNKN     = -1,     // Unknown error
    ENOENT    = -2,     // No such file or directory
    EACCES    = -13,    // Permission denied
    EEXIST    = -17,    // File/directory exists
    ENOTDIR   = -20,    // Not a directory
    EINVAL    = -22,    // Invalid argument
    ENOTEMPTY = -90,    // Directory not empty
}

pub type SyscallResult = ::core::result::Result<usize, Errno>;

pub fn convert_ret_code_to_syscall_result(ret_code: i64) -> SyscallResult {
    if ret_code < 0 {
        return Err(Errno::from(ret_code));
    } else {
        return Ok(ret_code as usize);
    }
}

pub fn convert_syscall_result_to_ret_code(syscall_result: SyscallResult) -> isize {
    let ret_val: i64;
    match syscall_result {
        Ok(t) => ret_val = t as i64,
        Err(e) => ret_val = e.into(),
    }
    ret_val as isize
}


