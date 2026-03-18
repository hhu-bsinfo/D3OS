#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::ptr::null;
use concurrent::{process, thread};
use naming::{mkfifo, open, read, write, ROOT, SHARED_PIPE};
use naming::shared_types::{OpenOptions, Capability};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};
use terminal::write::print;
use capabilities::{revoke_naming_object, share_naming_object};
use concurrent::thread::{current, sleep, Thread};

fn revoke_thread() {
    let Ok(file) = mkfifo("revoke", OpenOptions::READWRITE | OpenOptions::CREATE | OpenOptions::SHARE, ROOT) else {
        println!("---revoke_thread: failed to create pipe");
        return;
    };

    sleep(1000);
    share_naming_object(10, OpenOptions::all(), file); // Assuming main thread has ID 10
    // 
    // // Try to use the capability before it's revoked
    // // let mut buf = [0u8];
    // // let _read = read(file, &mut buf);
    // // println!("Read value before revoke: {}", buf[0]);
    // 
    sleep(4000);
    
    // Revoke the capability

    revoke_naming_object(10, file); // Assuming main thread has ID 10
    revoke_naming_object(12, file); // Assuming main thread has ID 10
    loop {
        
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    // Create the revoke thread
    let _revoke = thread::create(revoke_thread);
    
    thread::start_application("revoketest2", Vec::new());
    thread::start_application("revoketest3", Vec::new());
}