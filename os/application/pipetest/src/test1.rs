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
        println!("TEST1: open failed, error: {:?}", res);
        return;
    }
    let fh = res.unwrap();

    let wbuff: &[u8] = "hello world".as_bytes();
    let mut written: usize = 0;
    while written < wbuff.len() {
        match write(fh, &wbuff[written..]) {
            Ok(n) => {
                written += n;
            }
            Err(e) => {
                println!("TEST1: write failed, error: {:?}", e);
                break;
            }
        }
    }

    close(fh).expect("writer: failed to close pipe");
    println!("TEST1: wrote {} bytes", written);
}

fn reader_thread() {
    let res = open(FIFO_PATH, OpenOptions::READONLY);
    if res.is_err() {
        println!("TEST1: open failed, error: {:?}", res);
        return;
    }
    let fh = res.unwrap();

    let mut rbuff: [u8; 4096] = [0; 4096];
    let mut data: Vec<u8> = Vec::new();
    loop {
        match read(fh, &mut rbuff) {
            Ok(0) => {
                // EOF
                break;
            }
            Ok(n) => {
                data.extend_from_slice(&rbuff[..n]);
            }
            Err(e) => {
                println!("TEST1: read failed, error: {:?}", e);
                break;
            }
        }
    }

    match String::from_utf8(data) {
        Ok(s) => println!("TEST1: received string: \"{}\"", s),
        Err(e) => println!("TEST1: invalid UTF-8 received: {}", e),
    }
    close(fh).expect("Failed to close pipe");
    println!("TEST1: reader done");}

/*
fn reader_thread() {
    let thread = thread::current().unwrap();
    let reader_tid = thread.id();
    println!("r [{:02}]: start", reader_tid);
    let res = open(FIFO_PATH, OpenOptions::READONLY);
    if res.is_err() {
        println!("r [{:02}]: open failed, error: {:?}", reader_tid, res);
        return;
    }
    let fh = res.unwrap();

    println!("r [{:02}]: pipe handle = {:?}", reader_tid, fh);
    let mut rbuff: [u8; 1] = [0; 1];
    let mut cnt = 0;
    loop {
        let res = read(fh, &mut rbuff);
        if res.is_err() {
            println!("r [{:02}]: read failed, error: {:?}", reader_tid, res);
        } else {
            if rbuff[0].is_ascii() {
                let ch = rbuff[0] as char;
                println!("r [{:02}]: read one byte '{}', read = {}", reader_tid, ch, res.unwrap());
            } else {
                println!("r [{:02}]: read invalid data", reader_tid);
            }
        }
        cnt = cnt + 1;
        if cnt > NR_OF_ITERATIONS {
            break;
        }
//        concurrent::thread::sleep(1000);
    }

    close(fh).expect("Failed to close pipe");
    println!("r [{:02}]: end", reader_tid);
}
*/

/*fn writer_thread() {
    let thread = thread::current().unwrap();
    let writer_tid = thread.id();
    println!("w [{:02}]: start", writer_tid);
    let res = open(FIFO_PATH, OpenOptions::WRITEONLY);
    if res.is_err() {
        println!("w [{:02}]: open failed, error: {:?}", writer_tid, res);
        return;
    }
    let fh = res.unwrap();

    println!("w [{:02}]: pipe handle = {:?}", writer_tid, fh);
    let mut cnt = 0;
    let mut wbuff: [u8; 1] = [0; 1];
    let mut ch: u8 = b'A'; // start at ASCII 'A'
    loop {
        wbuff[0] = ch;
        let res = write(fh, &wbuff);
        if res.is_err() {
            println!("w [{:02}]: write failed, error: {:?}", writer_tid, res);
        } else {
            println!("w [{:02}]: wrote one byte = '{}'", writer_tid, ch as char);
            // Next letter
            ch = if ch == b'Z' {
                b'A' // wrap around after 'Z'
            } else {
                ch + 1
            };
        }
        cnt = cnt + 1;
        if cnt > NR_OF_ITERATIONS {
            break;
        }
//        concurrent::thread::sleep(1000);
    }

    close(fh).expect("Failed to close pipe");
    println!("w [{:02}]: end", writer_tid);
}
*/


pub fn test1_run() {
    println!("TEST1: regular reading & writing");

    let writer = thread::create(|| {
        writer_thread();
    });

    let reader = thread::create(|| {
        reader_thread();
    });

    let res = reader.unwrap().join();
    let res = writer.unwrap().join();

    println!("TEST1: OK");
}
