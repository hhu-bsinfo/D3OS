/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ App: shmtest                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Test application for shared memory.                                     ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Laurenz Maslo, Univ. Duesseldorf, 26.01.2026                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use concurrent::{process, shm, thread};
#[allow(unused_imports)]
use runtime::*;
use terminal::println;

fn reader_thread() {
    let my_tid = thread::current().unwrap().id();

    println!("TID [{}]: Reader thread", my_tid);

    let shm_id = shm::shm_open("shmtest", 1024, false).expect("shm_open failed (process 2)");
    println!("TID [{}]: Opened shared memory region", my_tid);

    let shm_ptr = shm::shm_attach(shm_id, true).expect("shm_attach failed (process 2)");
    println!("TID [{}]: Attached shared memory region at address {:p}", my_tid, shm_ptr);

    // read from shm
    unsafe {
        println!("TID [{}]: data read from shm: {}", my_tid,*shm_ptr);
    }

    shm::shm_detach(shm_ptr).expect("shm_detach failed (process 2)");
    println!("TID [{}]: Detached shared memory from current process.", my_tid);
}

#[unsafe(no_mangle)]
pub fn main() {
    let main_tid = thread::current().unwrap().id();

    println!("Testing shared memory");

    let shm_id = shm::shm_open("shmtest", 1024, true).expect("shm_open failed (process 1)");
    println!("TID [{}]: Created shared memory region", main_tid);

    let shm_ptr = shm::shm_attach(shm_id, false).expect("shm_attach failed (process 1)");
    println!("TID [{}]: Attached shared memory region at address {:p}", main_tid, shm_ptr);

    unsafe {
        *shm_ptr = 42;
    }
    println!("TID [{}]: Wrote to shared memory region", main_tid);

    match thread::create(reader_thread) {
        Some(t) => {
            println!("TID [{}]: Created reader thread, waiting for it to terminate.", main_tid);
            let _ = t.join();
        }
        None => println!("Failed to create reader_thread!"),
    }

    shm::shm_detach(shm_ptr).expect("shm_detach failed (process 1)");
    println!("TID [{}]: Detached shared memory from current process.", main_tid);

    shm::shm_unlink("shmtest").expect("shm_unlink failed");
    println!("TID [{}]: Unlink shared memory from current process.", main_tid);
}
