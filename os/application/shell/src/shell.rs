#![no_std]

extern crate alloc;

mod command_line;
mod controller;
mod executor;
mod lexer;
mod parser;

use concurrent::process;
use controller::Controller;
#[allow(unused_imports)]
use runtime::*;
use syscall::{SystemCall, syscall};
use terminal::read::read_mixed;

struct Shell {
    controller: Controller,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            controller: Controller::new(),
        }
    }

    pub fn init(&mut self) {
        self.controller.init();
    }

    pub fn run(&mut self) {
        loop {
            if syscall(SystemCall::TerminalTerminateOperator, &[0, 1]).unwrap() == 1 {
                process::exit();
            }

            let key = match read_mixed() {
                Some(key) => key,
                None => continue,
            };

            self.controller.run(key);
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut shell = Shell::new();
    shell.init();
    shell.run()
}
