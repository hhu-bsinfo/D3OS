#![no_std]

extern crate alloc;

use concurrent::{process, thread};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};


pub fn second_thread() {
    println!("Hello from the second thread!");
}

#[unsafe(no_mangle)]
pub fn main() {
    let process = process::current().unwrap();
    let thread = thread::current().unwrap();

    println!("Hello from Thread [{}] in Process [{}]!\n", thread.id(), process.id());

    println!("Arguments:");
    let args = env::args();
    for arg in args {
        println!("  {}", arg);
    }

    
    let v = thread::create(second_thread);
    if v.is_none() {
        println!("Failed to create second thread");
    } else {
        println!("Second thread created successfully.");
    }

}