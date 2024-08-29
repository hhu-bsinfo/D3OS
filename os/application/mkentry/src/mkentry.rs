#![feature(unsafe_attributes)]
#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use io::{print, println};
use naming::mkentry;

#[unsafe(no_mangle)]
pub fn main() {

    let res = mkentry("/home/schoettner", "test.txt", 1);

    println!("app: mkentry {:?}", res);
}