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
#![feature(c_variadic)]
#![allow(dead_code)]

extern crate alloc;

pub mod math;
pub mod stdlib;
pub mod string;
pub mod time;
pub mod ctype;
pub mod stdio;
pub mod errno;
pub mod sys;

use core::ffi::c_char;
use core::slice;
use crate::string::string::strlen;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn terminal_write(buffer: *const c_char) {
    let len = unsafe { strlen(buffer) };
    let bytes = unsafe { slice::from_raw_parts(buffer as *const u8, len) };
    if !terminal::write::write_bytes(bytes) {
        panic!("Error while writing to the terminal!");
    }
}

fn str_from_c_ptr<'a>(c_str: *const c_char) -> &'a str {
    use core::ffi::CStr;

    if c_str.is_null() {
        return "";
    }

    unsafe { CStr::from_ptr(c_str).to_str().expect("libc: Invalid UTF-8 string") }
}