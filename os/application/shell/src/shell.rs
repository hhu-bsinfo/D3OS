#![no_std]

extern crate alloc;

mod build_in;
mod context;
mod controller;
mod executor;
mod lexer;
mod parser;
mod service;
mod sub_module;

use context::Context;
use logger::info;
#[allow(unused_imports)]
use runtime::*;
use service::{command_line_service::CommandLineService, service::Service};
use terminal::{print, read::read_mixed};

struct Shell {
    // Context
    context: Context,
    // Required services
    command_line_service: CommandLineService,
    // Optional services
    // TODO
}

impl Shell {
    pub fn new() -> Self {
        Self {
            // Context
            context: Context::new(),
            // Required services
            command_line_service: CommandLineService::new(),
            // Optional services
            // TODO
        }
    }

    pub fn init(&mut self) {
        print!("\n");
        // self.controller.init();
    }

    pub fn run(&mut self) {
        loop {
            let key = match read_mixed() {
                Some(key) => key,
                None => continue,
            };

            self.context.event = key;
            info!("Read key: {:?}", self.context);
            self.command_line_service.run(&mut self.context);
            info!("Command line: {:?}", self.context);
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut shell = Shell::new();
    shell.init();
    shell.run()
}
