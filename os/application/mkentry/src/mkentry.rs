#![feature(unsafe_attributes)]
#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use io::{print, println};
use naming::mkentry;

#[unsafe(no_mangle)]
pub fn main() {

    let (r1,r2) = mkentry("/home/schoettner", "test.txt", 1);

    println!("mkentry: 0x{:x}, 0x{:x}", r1,r2);
}