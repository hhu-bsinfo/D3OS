#![no_std]

extern crate alloc;

use concurrent::{process, thread};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};

#[unsafe(no_mangle)]
pub fn main() {
    let process = process::current().unwrap();
    let thread = thread::current().unwrap();

    println!("Hello from Thread [{}] in Process [{}]!", thread.id(), process.id());

    let args = env::args();
    for (i, arg) in args.enumerate() {
        println!("Arg[{}]: {}", i, arg);
    }
}