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
use service::{
    command_line_service::CommandLineService, drawer_service::DrawerService,
    history_service::HistoryService, janitor_service::JanitorService, service::Service,
};
use terminal::read::read_mixed;

struct Shell {
    // Context
    context: Context,
    // Required services
    command_line_service: CommandLineService,
    drawer_service: DrawerService,
    janitor_service: JanitorService,
    // Optional services
    history_service: Option<HistoryService>,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            // Context
            context: Context::new(),
            // Required services
            command_line_service: CommandLineService::new(),
            drawer_service: DrawerService::new(),
            janitor_service: JanitorService::new(),
            // Optional services
            history_service: Some(HistoryService::new()),
        }
    }

    pub fn init(&mut self) {
        // print!("\n");
        // self.controller.init();
    }

    pub fn run(&mut self) {
        loop {
            let key = match read_mixed() {
                Some(key) => key,
                None => continue,
            };

            info!("Read key: {:?}", key);

            self.command_line_service.run(key, &mut self.context);
            info!("Command line: {:?}", self.context);

            self.history_service
                .as_mut()
                .unwrap() // TODO Check properly if enabled
                .run(key, &mut self.context);
            info!("History: {:?}", self.context);

            self.drawer_service.run(key, &mut self.context);
            info!("Drawer: {:?}", self.context);

            self.janitor_service.run(key, &mut self.context);
            info!("Janitor: {:?}", self.context);
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut shell = Shell::new();
    shell.init();
    shell.run()
}
