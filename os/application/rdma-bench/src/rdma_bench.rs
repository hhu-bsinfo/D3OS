#![no_std]

extern crate alloc;

mod bench;
mod cli;
mod client;
mod comm;
mod device;
mod error;
mod server;
mod transport;

use log::LevelFilter;
use cli::Cli;

#[unsafe(no_mangle)]
pub fn main() {
    terminal::init_logger();

    let cli = match Cli::parse() {
        Ok(cli) => cli,
        Err(message) => {
            terminal::println!("{}", message);
            return;
        }
    };

    let result = match cli {
        Cli::Server(args) => server::run(args),
        Cli::Client(args) => client::run(args),
    };

    if let Err(e) = result {
        terminal::println!("error: {:?}", e);
    }
}
