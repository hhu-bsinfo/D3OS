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
use concurrent::thread::{sleep, Thread, current};
#[unsafe(no_mangle)]
pub fn main() {
    println!("2nd Process Waiting for capability to be shared...");
    while get_naming_len() == 2 {
        sleep(1000);
    }

    let pipe = Capability::new(2); //Shared Cap at index 2
    let mut buf = [0u8; 5];
    
    let Ok(open_pipe) = open(pipe, OpenOptions::READWRITE) else {
        println!("Process 2: Failed to open shared pipe");
        return;
    };
    
    println!("Process 2: Successfully opened shared pipe");
    
    close(open_pipe);

    // let res = write(file, "Hello".as_ref());
    // if res.is_err() {
    //     println!("write error (2) = {:?}", res);
    // } else {
    //     println!("write successful, process 2");
    // }
    //
    // sleep(2000);
    //
    // let res = read(file, &mut buf);
    //
    // if res.is_err() {
    //     println!("read error (2) = {:?}", res);
    // } else {
    //     print!("read success (2):");
    //     println!("{:?}", &buf)
    // }

    sleep(1000);
    println!("Shared Capability with 3rd process");
    share_naming_object(12, OpenOptions::all(), pipe); // Assuming main thread has ID 12

    loop {
        let Ok(open_pipe) = open(pipe, OpenOptions::READWRITE) else {
            println!("Process 2 cap revoked");
            return;
        };
        if write(open_pipe, "Process 2: Capability revoked, write should fail".as_ref()).is_ok(){
            sleep(5000);
            println!("Process 2, waiting for capability to be revoked");
        } else {
            println!("Process 2 cap revoked")
        }
        close(open_pipe);
    }
}