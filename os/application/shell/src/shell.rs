#![no_std]

extern crate alloc;

mod input_reader;

use concurrent::process;
use input_reader::InputReader;
#[allow(unused_imports)]
use runtime::*;
use syscall::{SystemCall, syscall};

struct Shell {
    input_reader: InputReader,
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            input_reader: InputReader::new(),
        }
    }

    pub fn run(&self) {
        loop {
            if syscall(SystemCall::TerminalTerminateOperator, &[0, 1]).unwrap() == 1 {
                process::exit();
            }

            self.input_reader.read();
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let shell = Shell::new();
    shell.run()
}
