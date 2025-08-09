#![no_std]

extern crate alloc;

use concurrent::{process, thread};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};


fn second_thread() {
    let process = process::current().unwrap();
    let thread = thread::current().unwrap();

    println!("Hello from 2nd thread [{}] in process [{}]!", thread.id(), process.id());
    {
        let mut arr = [0; 1200];
        for i in 0..arr.len() {
            arr[i] = 1;
         }
         println!("2nd thread [{}] accessing array {}", thread.id(), arr[600]);
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let process = process::current().unwrap();
    let thread = thread::current().unwrap();

    
    let v = thread::create(|| {
       second_thread();
    });
    if let Some(v) = v {
        v.join();
    } else {
        println!("Failed to create second thread");
    }
    println!("main thread [{}] in process [{}]!", thread.id(), process.id());
 
}