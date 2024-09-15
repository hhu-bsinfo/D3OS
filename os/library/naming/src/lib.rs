/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for the naming service.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 15.9.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

use syscall::{return_vals::Errno, syscall, SystemCall};

pub fn mkdir(path: &str) -> Result<usize, Errno> {
    // Check if params are valid
    if path.is_empty() {
        Err(Errno::EINVAL) // Abort, if not
    } else {
        // params OK, do the syscall
        syscall(
            SystemCall::MkDir,
            &[
                path.as_bytes().as_ptr() as usize,
                path.len(),
            ],
        )
    }
}
