#![no_std]

extern crate alloc;

use concurrent::{process, thread};
#[allow(unused_imports)]
use runtime::*;
use terminal::println;


fn second_thread() {
    let process = process::current().unwrap();
    let thread = thread::current().unwrap();
    let start_time = thread.start_time();

    println!("Hello from second thread [{}] in process [{}] started at [{}]!", thread.id(), process.id(), start_time);

    let mut arr = [0; 1200];
    arr.fill(1);
    println!("Second thread [{}] accessing array[600]: [{}]", thread.id(), arr[600]);
}

#[unsafe(no_mangle)]
pub fn main() {
    let process = process::current().unwrap();
    let thread = thread::current().unwrap();
    let start_time = thread.start_time();

    println!("Hello from main thread [{}] in process [{}] started at [{}]!", thread.id(), process.id(), start_time);

    match thread::create(second_thread) {
        Some(t) => t.join(),
        None => println!("Failed to create anonymous thread!")
    }
}