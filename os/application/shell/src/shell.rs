#![no_std]

extern crate alloc;

mod build_in;
mod command_line;
mod controller;
mod executor;
mod lexer;
mod parser;
mod sub_module;

use concurrent::process;
use controller::Controller;
#[allow(unused_imports)]
use runtime::*;
use syscall::{SystemCall, syscall};
use terminal::{print, read::read_mixed};

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
        print!("\n");
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

// TODO BUG typed command at cursor disappears when hitting enter while cursor is not at the end of line
