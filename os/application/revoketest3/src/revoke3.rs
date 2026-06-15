#![no_std]

extern crate alloc;

use alloc::string::String;
use core::ptr::null;
use concurrent::{process, thread};
use naming::{close, mkfifo, open, read, write, ROOT};
use naming::shared_types::{Capability, OpenOptions};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};
use terminal::write::print;
use capabilities::{get_naming_len, revoke_naming_object, share_naming_object};
use concurrent::process::Process;
use concurrent::thread::{sleep, Thread, current};
#[unsafe(no_mangle)]
pub fn main() {
    println!("3rd Process Waiting for capability to be shared...");
    while get_naming_len() < 3 {
        sleep(100);
    }

    let pipe = Capability::new(2); //Shared Cap at index 2
    let mut buf = [0u8; 5];

    let Ok(open_pipe) = open(pipe, OpenOptions::READWRITE) else {
        println!("Process 3: Failed to open shared pipe");
        return;
    };

    println!("Process 3: Successfully opened shared pipe");

    // let res = write(file, "Hello".as_ref());
    // if res.is_err() {
    //     println!("write error = {:?}", res);
    // } else {
    //     println!("write successful, process 3");
    // }
    //
    // sleep(3000);
    //
    // let res = read(file, &mut buf);
    //
    // if res.is_err() {
    //     println!("read error (3) = {:?}", res);
    // } else {
    //     print!("read success (3):");
    //     println!("{:?}", &buf)
    // }
    //
    // while get_naming_len() < 4 {
    //     sleep(100);
    // }
    //
    // println!("Process 3, {} caps", get_naming_len());
    //
    // let file2 = Capability::new(4); //Shared Cap at index 3
    // let mut buf = [0u8; 5];
    //
    // let res = write(file2, "Hello".as_ref());
    // if res.is_err() {
    //     println!("write error (3) = {:?}", res);
    // } else {
    //     println!("write successful, process 3");
    // }
    //
    // sleep(3000);
    //
    // let res = read(file2, &mut buf);
    //
    // if res.is_err() {
    //     println!("read error (3) = {:?}", res);
    // } else {
    //     print!("read success (3):");
    //     println!("{:?}", &buf)
    // }

    loop {
        let Ok(open_pipe) = open(pipe, OpenOptions::READWRITE) else { 
            println!("Process 3 cap revoked");
            return;
        };
        if write(open_pipe, "Process 3: Capability revoked, write should fail".as_ref()).is_ok(){
            sleep(5000);
            println!("Process 3, waiting for capability to be revoked");
        } else {
            println!("Process 3 cap revoked")
        }
        close(open_pipe);
    }
}