#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};
use naming::mkdir;

#[unsafe(no_mangle)]
pub fn main() {
    let args = env::args();
    for (i, arg) in args.enumerate() {
        println!("Arg[{}]: {}", i, arg);
    }

    let res = mkdir("/home/schoettner");

    println!("app: mkdir {:?}", res);
}