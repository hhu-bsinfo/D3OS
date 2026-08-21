#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use terminal::println;

use logger::*;
use log::{error, info, warn};
use log::LevelFilter;

static LOGGER: Logger = Logger::new();

#[unsafe(no_mangle)]
pub fn main() {
    println!("Demo app showing how to use the logging macros!");
    println!("Log output will be dumped on serial port using syscalls");

    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info));

    error!("logtest: error");
    warn!("logtest: warning");
    info!("logtest: info"); 
}
