/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Runtime functions for C-applications.                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Gökhan Cöpcü                                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

#![cfg_attr(not(test), no_std)]
#![feature(c_size_t)]
#![allow(dead_code)]

extern crate alloc;

pub mod math;
pub mod stdlib;
pub mod string;
pub mod time;

use core::ffi::c_char;
use syscall::{syscall, SystemCall};
use crate::string::string::strlen;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn terminal_write(buffer: *const c_char) {
    let res = syscall(SystemCall::TerminalWrite, &[buffer as usize, unsafe { strlen(buffer) }]);
    if res.is_err() {
        panic!("Error while writing to the terminal!");
    }
}