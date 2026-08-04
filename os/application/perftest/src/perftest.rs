#![no_std]

use log::error;
use runtime::env::args;

mod read_bw;
mod comm;

extern crate alloc;

#[unsafe(no_mangle)]
pub fn main() {
    let args = args();
    let config = read_bw::Config::default();
    match read_bw::run(config) {
        Ok(_) => {}
        Err(e) => error!("failed to run bandwidth: {}", e),
    }
}
