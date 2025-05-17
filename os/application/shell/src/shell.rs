#![no_std]

extern crate alloc;

mod executor;
mod input_reader;
pub mod module;
mod parser;
pub mod state;

use core::cell::RefCell;

use alloc::rc::Rc;
use concurrent::process;
use executor::executor::Executor;
use input_reader::input_reader::InputReader;
use module::Module;
use parser::lexical_parser::LexicalParser;
#[allow(unused_imports)]
use runtime::*;
use state::State;
use syscall::{SystemCall, syscall};

struct Shell {
    state: Rc<RefCell<State>>,
    input_reader: InputReader,
    parser: LexicalParser,
    executor: Executor,
}

impl Shell {
    pub fn new() -> Self {
        let state = Rc::new(RefCell::new(State::new()));
        let input_reader = InputReader::new(state.clone());
        let parser = LexicalParser::new(state.clone());
        let executor = Executor::new(state.clone());

        Self {
            state,
            input_reader,
            parser,
            executor,
        }
    }

    pub fn run(&mut self) {
        loop {
            if syscall(SystemCall::TerminalTerminateOperator, &[0, 1]).unwrap() == 1 {
                process::exit();
            }

            self.input_reader.run();
            self.parser.run();
            self.executor.run();
            self.state.borrow_mut().clear();
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut shell = Shell::new();
    shell.run()
}
