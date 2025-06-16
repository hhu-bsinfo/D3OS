#![no_std]

extern crate alloc;

use concurrent::{process, thread};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};


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

    
    let v = thread::create(|| {
        println!("Hello from the second thread!");
    });
    if let Some(v) = v {
        println!("Second thread created successfully.");
        v.join();
    } else {
        println!("Failed to create second thread");
    }

}