#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};
use naming::mkentry;

#[unsafe(no_mangle)]
pub fn main() {
    let args = env::args();
    for (i, arg) in args.enumerate() {
        println!("Arg[{}]: {}", i, arg);
    }

    let res = mkentry("/home/schoettner", "test.txt", 1);

    println!("app: mkentry {:?}", res);
}