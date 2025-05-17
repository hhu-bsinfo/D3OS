#![no_std]

extern crate alloc;

use concurrent::process;
#[allow(unused_imports)]
use runtime::*;
use syscall::{SystemCall, syscall};
use terminal::read::read;

#[unsafe(no_mangle)]
pub fn main() {
    loop {
        if syscall(SystemCall::TerminalTerminateOperator, &[0, 1]).unwrap() == 1 {
            process::exit();
        }

        read();
    }
}
