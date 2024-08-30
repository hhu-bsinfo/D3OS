#![no_std]

extern crate alloc;

use concurrent::{process, thread};
#[allow(unused_imports)]
use runtime::*;
use io::{print, println};

#[unsafe(no_mangle)]
pub fn main() {
    let process = process::current();
    let thread = thread::current();
    println!("Hello from Thread [{}] in Process [{}]!", thread.id(), process.id());

    let args = env::args();
    for (i, arg) in args.enumerate() {
        println!("Arg[{}]: {}", i, arg);
    }
}