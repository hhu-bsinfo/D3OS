#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use concurrent::process;

#[allow(unused_imports)]
use runtime::*;
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
