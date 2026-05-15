#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use concurrent::thread;
use terminal::println;
use time::date;

fn worker_loop() {
    loop {
        
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    for i in 0..40 {
        //println!("Creating worker thread with ID: {}", i);
        let _ = thread::create(worker_loop);
    }


}
