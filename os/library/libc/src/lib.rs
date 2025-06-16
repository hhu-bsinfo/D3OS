/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Runtime functions for C-applications.                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 31.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

use stream::strlen;
use syscall::{syscall, SystemCall};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn terminal_write(buffer: *const u8) {
    let res = syscall(SystemCall::TerminalWrite, &[buffer as usize, unsafe { strlen(buffer) }]);
    if res.is_err() {
        panic!("Error while writing to the terminal!");
    }
}