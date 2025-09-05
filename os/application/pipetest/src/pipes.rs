#![no_std]

extern crate alloc;

use naming::shared_types::OpenOptions;
use naming::{close, mkfifo, open, read, write};

use concurrent::thread;
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};

const PIPE: &str = "/mypipe";
const NR_OF_ITERATIONS: u32 = 6;

fn writer_thread() {
    let thread = thread::current().unwrap();
    println!("writer_thread (tid={}): start", thread.id());
    let res = open("/mypipe", OpenOptions::WRITEONLY);
    if res.is_err() {
        println!("writer_thread: open failed, error: {:?}", res);
        return;
    }
    let fh = res.unwrap();

    let mut cnt = 0;
    let mut wbuff: [u8; 1] = [0; 1];
    let mut ch: u8 = b'A'; // start at ASCII 'A'
    loop {
        wbuff[0] = ch;
        let res = write(fh, &wbuff);
        if res.is_err() {
            println!("writer_thread: write failed, error: {:?}", res);
        } else {
            println!("writer_thread: wrote one byte = '{}'", ch as char);

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

    close(fh);
    println!("writer_thread: end");
}

fn reader_thread() {
    let thread = thread::current().unwrap();
    println!("reader_thread (tid={}): start", thread.id());
    let res = open("/mypipe", OpenOptions::READONLY);
    if res.is_err() {
        println!("reader_thread: open failed, error: {:?}", res);
        return;
    }
    let fh = res.unwrap();

    let mut rbuff: [u8; 1] = [0; 1];
    let mut cnt = 0;
    loop {
        let res = read(fh, &mut rbuff);
        if res.is_err() {
            println!("reader_thread: read failed, error: {:?}", res);
        } else {
            if rbuff[0].is_ascii() {
                let ch = rbuff[0] as char;
                println!("reader_thread: read one byte '{}', read = {}", ch, res.unwrap());
            } else {
                println!("reader_thread: read invalid data");
            }
        }
        cnt = cnt + 1;
        if cnt > NR_OF_ITERATIONS {
            break;
        }
//        concurrent::thread::sleep(1000);
    }

    close(fh);
    println!("reader_thread: end");
}

#[unsafe(no_mangle)]
pub fn main() {
    println!("named pipe demo: start");

    let res = mkfifo("/mypipe");
    if res.is_err() {
        println!("mkfifo failed, error: {:?}", res);
        return;
    }
    println!("mkfifo: ok");

    let writer = thread::create(|| {
        writer_thread();
    });
    let reader = thread::create(|| {
        reader_thread();
    });

    if let Some(w) = writer {
        w.join();
    }
    if let Some(r) = reader {
        r.join();
    }

    println!("named pipe demo: done");
}
