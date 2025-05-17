#![no_std]

extern crate alloc;

mod input_reader;
mod parser;

use concurrent::process;
use input_reader::InputReader;
use parser::lexical_parser::LexicalParser;
#[allow(unused_imports)]
use runtime::*;
use syscall::{SystemCall, syscall};

struct Shell {
    input_reader: InputReader,
    parser: LexicalParser,
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            input_reader: InputReader::new(),
            parser: LexicalParser::new(),
        }
    }

    pub fn run(&mut self) {
        loop {
            if syscall(SystemCall::TerminalTerminateOperator, &[0, 1]).unwrap() == 1 {
                process::exit();
            }

            self.input_reader.read(&mut self.parser);
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut shell = Shell::new();
    shell.run()
}
