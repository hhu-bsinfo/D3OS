#![no_std]

extern crate alloc;

use alloc::string::ToString;
use concurrent::{process, thread};
#[allow(unused_imports)]
use runtime::*;
use runtime::env::args;
use terminal::println;


fn thread_fn() {
    let process = process::current();
    let thread = thread::current().unwrap();
    let start_time = thread.start_time();

    println!("Hello from thread [{}] in process [{}] started at [{}]!", thread.id(), process.id(), start_time);

    let mut arr = [0; 16384];
    arr.fill(1);

    println!("Thread [{}] accessing arr[1797]: [{}]", thread.id(), arr[1797]);
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut it = args().skip(1);

    let num_threads: usize = it.next()
        .unwrap_or("1".to_string())
        .parse()
        .expect("Failed to parse number of threads argument!");

    let panic_mode = false;
    let process = process::current();
    let thread = thread::current().unwrap();
    let start_time = thread.start_time();
    

    println!("Hello from main thread with ID [{}] in process [{}] started at [{}]!", thread.id(), process.id(), start_time);

    for _ in 0..num_threads {
        let t = if panic_mode {
            thread::create(|| panic!("threadtest: intentional panic"))
        } 
        else {
            thread::create(thread_fn)
        };

        match t {
            Some(t) => match t.join() {
                Ok(code) => println!("Joined thread [{}] with exit code [{}]", t.id(), code),
                Err(e) => println!("Join failed for thread [{}]: {:?}", t.id(), e),
            },
            None => println!("Failed to create anonymous thread!"),
        }
    }
}