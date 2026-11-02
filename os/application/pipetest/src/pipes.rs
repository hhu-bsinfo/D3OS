#![no_std]

extern crate alloc;

mod test1;
mod test2;
mod test3;
mod test4;

#[allow(unused_imports, reason = "contains global_allocator")]
use runtime::*;
use alloc::string::String;
use alloc::vec::Vec;

use naming::mkfifo;
use syscall::return_vals::Errno;

use concurrent::{thread,process};
use terminal::println;

const FIFO_PATH: &str = "/mypipe";

#[unsafe(no_mangle)]
pub fn main() {
    // TEST4 reuses this binary as its own child process: if argv[1] selects a
    // child role, run it and exit instead of the parent test flow.
    let args: Vec<String> = runtime::env::args().collect();
    if let Some(role) = args.get(1) {
        if test4::child_dispatch(role.as_str()) {
            return;
        }
    }

    let process = process::current();
    let thread = thread::current().unwrap();
    let main_tid = thread.id();
    println!("MAIN: pid={}, tid={}", process.id(), main_tid);

    let res = mkfifo(FIFO_PATH);
    match res {
        Ok(_) => println!("MAIN:  mkfifo created"),
        Err(e) => {
            if e == Errno::EEXIST {
                println!("MAIN:  mkfifo pipe already exists");
            } else {
                println!("MAIN:  mkfifo failed, error: {:?}", e);
                return;
            }
        }
    }

    test1::test1_run();

    test2::test2_run();

    test3::test3_run();

    test4::test4_run();
}
