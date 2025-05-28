#![no_std]

extern crate alloc;

mod build_in;
mod context;
mod controller;
mod executable;
mod service;
mod sub_module;
mod sub_service;

use core::cell::RefCell;

use alloc::rc::Rc;
use context::Context;
use logger::info;
#[allow(unused_imports)]
use runtime::*;
use service::{
    command_line_service::CommandLineService, drawer_service::DrawerService,
    executor_service::ExecutorService, history_service::HistoryService,
    janitor_service::JanitorService, lexer_service::LexerService, parser_service::ParserService,
    service::Service,
};
use sub_service::alias_sub_service::AliasSubService;
use terminal::read::read_mixed;

struct Shell {
    // Context
    context: Context,
    // Required services
    command_line_service: CommandLineService,
    lexer_service: LexerService,
    drawer_service: DrawerService,
    parser_service: ParserService,
    janitor_service: JanitorService,
    executor_service: ExecutorService,
    // Optional services
    history_service: Option<HistoryService>,
    alias_service: Rc<RefCell<AliasSubService>>,
}

impl Shell {
    pub fn new() -> Self {
        let alias_service = Rc::new(RefCell::new(AliasSubService::new()));
        Self {
            // Context
            context: Context::new(),
            // Required services
            command_line_service: CommandLineService::new(),
            lexer_service: LexerService::new(alias_service.clone()),
            drawer_service: DrawerService::new(),
            parser_service: ParserService::new(),
            executor_service: ExecutorService::new(alias_service.clone()),
            janitor_service: JanitorService::new(),
            // Optional services
            history_service: Some(HistoryService::new()),
            alias_service,
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
            info!(
                "After Command line: [ cursor: {:?}, dirty_offset: {:?}, line: {:?} ]",
                self.context.cursor_position, self.context.dirty_offset, self.context.line
            );

            self.history_service
                .as_mut()
                .unwrap() // TODO Check properly if enabled
                .run(key, &mut self.context);
            // info!(
            //     "After History: [ cursor: {:?}, dirty_offset: {:?}, line: {:?} ]",
            //     self.context.cursor_position, self.context.dirty_offset, self.context.line
            // );

            self.lexer_service.run(key, &mut self.context);

            self.drawer_service.run(key, &mut self.context);

            self.parser_service.run(key, &mut self.context);

            self.executor_service.run(key, &mut self.context);

            self.janitor_service.run(key, &mut self.context);
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut shell = Shell::new();
    shell.init();
    shell.run()
}
