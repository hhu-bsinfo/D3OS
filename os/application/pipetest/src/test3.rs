extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use naming::shared_types::OpenOptions;
use naming::{close, open, read, write};
use syscall::return_vals::Errno;

use concurrent::thread;
#[allow(unused_imports)]
use runtime::*;
use terminal::println;

const FIFO_PATH: &str = "/mypipe";

fn reader(label: &'static str) {
    let fh = match open(FIFO_PATH, OpenOptions::READONLY) {
        Ok(fh) => fh,
        Err(e) => {
            println!("TEST3: {} open failed, error: {:?}", label, e);
            return;
        }
    };

    let mut rbuff: [u8; 4096] = [0; 4096];
    let mut data: Vec<u8> = Vec::new();
    loop {
        match read(fh, &mut rbuff) {
            Ok(0) => break, // EOF: all writers have closed
            Ok(n) => data.extend_from_slice(&rbuff[..n]),
            Err(e) => {
                println!("TEST3: {} read failed, error: {:?}", label, e);
                break;
            }
        }
    }

    close(fh).expect("reader: failed to close pipe");
    match String::from_utf8(data) {
        Ok(s) => println!("TEST3: {} received \"{}\"", label, s),
        Err(e) => println!("TEST3: {} invalid UTF-8: {}", label, e),
    }
}

fn writer(label: &'static str, data: &'static [u8]) {
    let fh = match open(FIFO_PATH, OpenOptions::WRITEONLY) {
        Ok(fh) => fh,
        Err(e) => {
            println!("TEST3: {} open failed, error: {:?}", label, e);
            return;
        }
    };

    // Sleep around the write so every writer is open before any closes; this
    // keeps the reader from hitting EOF until the last writer is gone.
    thread::sleep(100);
    let mut written = 0;
    while written < data.len() {
        match write(fh, &data[written..]) {
            Ok(n) => written += n,
            Err(e) => {
                println!("TEST3: {} write failed, error: {:?}", label, e);
                break;
            }
        }
    }
    thread::sleep(100);

    close(fh).expect("writer: failed to close pipe");
    println!("TEST3: {} wrote {} bytes", label, written);
}

fn shared_stream() {
    println!("TEST3: two readers and two writers share the pipe");
    let reader_a = thread::create(|| reader("reader-a")).unwrap();
    let reader_b = thread::create(|| reader("reader-b")).unwrap();
    let writer_a = thread::create(|| writer("writer-a", b"alpha")).unwrap();
    let writer_b = thread::create(|| writer("writer-b", b"bravo")).unwrap();

    let _ = writer_a.join();
    let _ = writer_b.join();
    let _ = reader_a.join();
    let _ = reader_b.join();
}

fn last_writer_eof() {
    println!("TEST3: reader sees EOF only after the last writer closes");
    let reader = thread::create(|| reader("reader")).unwrap();
    let writer_a = thread::create(|| writer("writer-a", b"first ")).unwrap();
    let writer_b = thread::create(|| writer("writer-b", b"second")).unwrap();

    let _ = writer_a.join();
    let _ = writer_b.join();
    let _ = reader.join();
}

fn last_reader_epipe() {
    println!("TEST3: writer gets EPIPE after the last reader closes");

    let writer = thread::create(|| {
        let fh = match open(FIFO_PATH, OpenOptions::WRITEONLY) {
            Ok(fh) => fh,
            Err(e) => {
                println!("TEST3: writer open failed, error: {:?}", e);
                return;
            }
        };

        thread::sleep(100); // let the reader close first
        match write(fh, b"x") {
            Err(Errno::EPIPE) => println!("TEST3: writer got EPIPE as expected"),
            Ok(n) => println!("TEST3: writer unexpectedly wrote {} bytes", n),
            Err(e) => println!("TEST3: writer got unexpected error: {:?}", e),
        }
        close(fh).expect("epipe writer: failed to close pipe");
    })
    .unwrap();

    // open() rendezvous: this returns once the writer is present; closing
    // immediately leaves the pipe with no readers.
    let reader_fh = open(FIFO_PATH, OpenOptions::READONLY).expect("epipe reader: failed to open pipe");
    close(reader_fh).expect("epipe reader: failed to close pipe");
    let _ = writer.join();
}

pub fn test3_run() {
    shared_stream();
    last_writer_eof();
    last_reader_epipe();
    println!("TEST3: OK");
}
