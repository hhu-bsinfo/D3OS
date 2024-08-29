/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for the naming service.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 29.8.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use num_enum::{FromPrimitive,IntoPrimitive};

#[derive(Debug, Copy, Clone, Eq, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(usize)]
pub enum Errno {
    #[num_enum(default)]
    ENOENT = 2, 	    /* No such file or directory */
    EACCES = 13,	    /* Permission denied */
    EEXIST = 17,	    /* File/directory exists */
    ENOTDIR = 20,	    /* Not a directory */
    EINVAL = 22,	    /* Invalid argument */
    ENOTEMPTY = 90,	    /* Directory not empty */
}

pub type SyscallResult<T> = ::core::result::Result<T, Errno>;
pub type Result<T> = ::core::result::Result<T, Errno>;
