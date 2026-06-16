//! ip – show the current IP address

#![no_std]
extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use network::get_ip_addresses;
use terminal::println;

#[unsafe(no_mangle)]
fn main() {
    // ignore all args for now

    for ip in get_ip_addresses() {
        println!("{}", ip)
    }
}
