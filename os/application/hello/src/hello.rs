#![no_std]

use runtime::*;
use thread::{process_id, thread_id};

#[no_mangle]
pub fn main() {
    println!("Hello from Thread [{}] in Process [{}]!", thread_id(), process_id());
}