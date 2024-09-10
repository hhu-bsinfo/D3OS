/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for the naming service.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 30.8.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

use syscall::{return_vals::Errno, syscall, SystemCall};

pub fn mkentry(path: &str, name: &str, data: usize) -> Result<usize, Errno> {
    // Check if params are valid
    if path.is_empty() || name.is_empty() {
        Err(Errno::EINVAL) // Abort, if not
    } else {
        // params OK, do the syscall
        syscall(
            SystemCall::Mkentry,
            &[
                path.as_bytes().as_ptr() as usize,
                path.len(),
                name.as_bytes().as_ptr() as usize,
                name.len(),
                data, // place holder, to be replaced by pointer to container
            ],
        )
    }
}
