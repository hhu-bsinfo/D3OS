#![no_std]

extern crate alloc;

#[allow(unused_imports, reason = "contains global_allocator")]
use runtime::*;

use concurrent::process;
use terminal::println;

#[unsafe(no_mangle)]
pub fn main() {

    let mut buff: [u8; 2048] = [0; 2048];
    let res = process::ps(&mut buff);

    if let Ok(len) = res {
        // Treat buffer as UTF-8 text (like /proc)
        let s = core::str::from_utf8(&buff[..len]).unwrap_or("<invalid utf-8>");
        println!("{}", s);
    }
}
