#![no_std]

extern crate alloc;

mod executor;
mod input_reader;
mod parser;

use concurrent::process;
use executor::executor::Executor;
use input_reader::InputReader;
use parser::lexical_parser::LexicalParser;
#[allow(unused_imports)]
use runtime::*;
use syscall::{SystemCall, syscall};

struct Shell {
    input_reader: InputReader,
    parser: LexicalParser,
    executor: Executor,
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            input_reader: InputReader::new(),
            parser: LexicalParser::new(),
            executor: Executor::new(),
        }
    }

    pub fn run(&mut self) {
        loop {
            if syscall(SystemCall::TerminalTerminateOperator, &[0, 1]).unwrap() == 1 {
                process::exit();
            }

            self.input_reader.read(&mut self.parser, &self.executor);
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut shell = Shell::new();
    shell.run()
}
