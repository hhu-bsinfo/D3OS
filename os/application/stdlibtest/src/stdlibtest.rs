//#![no_std]

//extern crate alloc;


#[allow(unused_imports)]
//use runtime::*;
//use terminal::println;
//use std::io::Write;
//use std::stdio::*;
use std::io::Write;


#[unsafe(no_mangle)]
pub fn main() {
    let mut buffer = std::io::stdout();
    let x = buffer.write(&"12345".as_bytes());
}