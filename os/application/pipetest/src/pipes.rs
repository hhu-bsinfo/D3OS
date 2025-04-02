#![no_std]

extern crate alloc;

use naming::shared_types::{OpenOptions, Capability};
use naming::{mkfifo, open, read, write, ROOT, SHARED_PIPE};

use concurrent::thread;
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};
use capabilities::*;
use concurrent::thread::{current, sleep};
use terminal::write::print;

const PIPE: &str = "/mypipe";
const NR_OF_ITERATIONS: u32 = 6;

fn writer_thread() {
    sleep(500);

    println!("---writer_thread: start, id {}", current().unwrap().id());
    let thread = thread::current().unwrap();
    let mut buff= [0;1];
    //let res = read(SHARED_PIPE, &mut buff);
    let pipe_cap = Capability::new(3); //receive the cap number

    let Ok(open_pipe) = open(pipe_cap, OpenOptions::READWRITE) else {
        println!("open failed");
        return;
    };
    
    println!("---writer_thread: got capability handle = {:?}", pipe_cap);

    let mut cnt = 0;
    let mut wbuff: [u8; 1] = [0; 1];
    let mut ch: u8 = b'A'; // start at ASCII 'A'
    loop {
        wbuff[0] = ch;
        let res = write(open_pipe, &wbuff);

        if res.is_err() {
            println!("---writer_thread: write failed, error: {:?}", res);
        } else {
            println!("---writer_thread: wrote one byte = '{}'", ch as char);

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
       concurrent::thread::sleep(1000);
    }

    // close(cap_handle);
    println!("---writer_thread: end");
}

fn reader_thread() {
    sleep(500);

    // let thread = thread::current().unwrap();
    // println!("reader_thread (tid={}): start", thread.id());
    // let res = open("/mypipe", OpenOptions::READONLY);
    // if res.is_err() {
    //     println!("reader_thread: open failed, error: {:?}", res);
    //     return;
    // }
    let mut buff= [0;1];
    //let res = read(SHARED_PIPE, &mut buff);
    let pipe_cap = Capability::new(2); // buff[0] as usize; //receive the cap number

    let Ok(open_pipe) = open(pipe_cap, OpenOptions::READWRITE) else {
        println!("open failed");
        return;
    };


    let mut rbuff: [u8; 1] = [0; 1];
    let mut cnt = 0;
    loop {
        let res = read(open_pipe, &mut rbuff);
        if res.is_err() {
            println!("+++reader_thread: read failed, error: {:?}", res);
        } else {
            if rbuff[0].is_ascii() {
                let ch = rbuff[0] as char;
                println!("+++reader_thread: read one byte '{}', read = {}", ch, res.unwrap());
            } else {
                println!("+++reader_thread: read invalid data");
            }
        }
        cnt = cnt + 1;
        if cnt > NR_OF_ITERATIONS {
            break;
        }
       concurrent::thread::sleep(1000);
    }

    //close(open_pipe);
    println!("+++reader_thread: end");
}

#[unsafe(no_mangle)]
pub fn main() {
    println!("named pipe demo: start, id {}", current().unwrap().id());

    println!("got root capability");

    // debug_print_caps(thread::current().unwrap().id());

    let res = mkfifo("mypipe", OpenOptions::READWRITE | OpenOptions::SHARE, ROOT);
    if res.is_err() {
        println!("mkfifo failed, error: {:?}", res);
        return;
    }
    let pipe_cap = res.unwrap();
    println!("mkfifo: ok, cap_handle = {:?}", pipe_cap);

    let Ok(open_pipe) = open(pipe_cap, OpenOptions::READWRITE) else {
        println!("open failed");
        return;
    };

    write(open_pipe, b"Hello from main thread!").unwrap();
    let buf = &mut [0u8; 23];
    read(open_pipe, buf).unwrap();

    // Print individual bytes as characters
    print!("Read: ");
    for &byte in buf.iter() {
        if byte != 0 {  // Skip null bytes
            print!("{}", byte as char);
        }
    }
    println!("");

    // share_naming_object(current().unwrap().id(), pipe_cap); //share with self to test

    let writer = thread::create(|| {
        writer_thread();
    });
    
    if let Some(w) = writer {

        println!("Starting writer, id {}", w.id());
        let num = share_naming_object(w.id(), OpenOptions::all(), pipe_cap);
        if num < 0 {
            println!("Failed to share pipe cap with writer thread (id {})", w.id());
            return;
        }

        println!("Shared pipe cap {} with writer thread {}", num, w.id());
        let buff= [num as u8];
        let res = write(SHARED_PIPE, &buff); 
        println!("Sent pipe cap to writer thread {}", w.id());
        w.join()
    }

    println!("Writer done, starting reader");
    
    let reader = thread::create(|| {
        reader_thread();
    });
    
    if let Some(r) = reader {
        let num = share_naming_object(r.id(), OpenOptions::all(), pipe_cap);
        let buff= [num as u8];
        let res = write(SHARED_PIPE, &buff);
        r.join();
    }

    println!("named pipe demo: done");
}
