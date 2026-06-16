#![no_std]

extern crate alloc;

use naming::shared_types::OpenOptions;
use naming::{close, mkfifo, open, read, write};
use syscall::return_vals::Errno;
use alloc::vec::Vec;
use alloc::string::String;

use concurrent::{thread,process};
#[allow(unused_imports)]
use runtime::*;
use terminal::println;

const FIFO_PATH: &str = "/mypipe";

fn writer_thread() {
    let res = open(FIFO_PATH, OpenOptions::WRITEONLY);
    if res.is_err() {
        println!("TEST2: open failed, error: {:?}", res);
        return;
    }
    let fh = res.unwrap();
    
    close(fh).expect("writer: failed to close pipe");
    println!("TEST2: wrote nothing, terminating");
}

fn reader_thread() {
    let res = open(FIFO_PATH, OpenOptions::READONLY);
    if res.is_err() {
        println!("TEST2: open failed, error: {:?}", res);
        return;
    }
    let fh = res.unwrap();


    concurrent::thread::sleep(1000);
    
    let mut rbuff: [u8; 4096] = [0; 4096];
    let mut data: Vec<u8> = Vec::new();
    loop {
        match read(fh, &mut rbuff) {
            Ok(0) => {
                println!("TEST2: EOF");
                // EOF
                break;
            }
            Ok(n) => {
                data.extend_from_slice(&rbuff[..n]);
            }
            Err(e) => {
                println!("TEST2: read failed, error: {:?}", e);
                break;
            }
        }
    }

    close(fh).expect("Failed to close pipe");
    println!("TEST2: reader done");
}

pub fn test2_run() {
    println!("TEST2: writer terminates before reader");

    let writer = thread::create(|| {
        writer_thread();
    });

    let reader = thread::create(|| {
        reader_thread();
    });

    let res = reader.unwrap().join();
    let res = writer.unwrap().join();

    println!("TEST2: OK");
}
