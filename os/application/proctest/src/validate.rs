#![no_std]

extern crate alloc;

use naming::shared_types::{OpenOptions};
#[allow(unused_imports)]
use runtime::*;
use terminal::println;

use alloc::format;

// test if /proc exists
fn test_proc_root() {
    match naming::open("/proc", OpenOptions::READONLY) {
        Ok(fd) => println!("OK: /proc opened, fd={}", fd),
        Err(e) => println!("FAIL: open(/proc) -> {:?}", e),
    }
}

// test if /proc/<pid> exisits
fn test_proc_pid(pid: usize) {
    let path = format!("/proc/{}", pid);
    match naming::open(&path, OpenOptions::READONLY) {
        Ok(fd) => println!("OK: {} opened (fd={})", path, fd),
        Err(e) => println!("FAIL: open({}) -> {:?}", path, e),
    }
}

// open the status file 
fn read_proc_status(pid: usize) {
    println!("NOW READING PROC_PID_STATUS");
    let path = format!("/proc/{}/status", pid);
    let fd = naming::open(&path, OpenOptions::READONLY).expect("open failed");

    let mut buf = [0u8; 256];
    let n = naming::read(fd, &mut buf).expect("read failed");

    let s = core::str::from_utf8(&buf[..n]).unwrap_or("<non-utf8>");
    println!("--- {} ({} bytes) ---", path, n);
    println!("{}", s);

    let _ = naming::close(fd);
}

// open the ps file
fn read_proc_ps() {
    println!("NOW READING PROC_PS");
    let path = "/proc/ps";
    let fd = naming::open(&path, OpenOptions::READONLY).expect("open failed");

    let mut buf = [0u8; 256];
    let n = naming::read(fd, &mut buf).expect("read failed");

    let s = core::str::from_utf8(&buf[..n]).unwrap_or("<non-utf8>");
    println!("--- {} ({} bytes) ---", path, n);
    println!("{}", s);
    
    let _ = naming::close(fd);
}

// open the ticks file
fn read_proc_ticks() {
    println!("NOW READING PROC_TICKS");
    let path = "/proc/ticks";
    let fd = naming::open(&path, OpenOptions::READONLY).expect("open failed");

    let mut buf = [0u8; 256];
    let n = naming::read(fd, &mut buf).expect("read failed");

    let s = core::str::from_utf8(&buf[..n]).unwrap_or("<non-utf8>");
    println!("--- {} ({} bytes) ---", path, n);
    println!("{}", s);
    
    let _ = naming::close(fd);
}

// open the meminfo file
fn read_proc_meminfo() {
    println!("NOW READING PROC_MEMINFO");
    let path = "/proc/meminfo";
    let fd = naming::open(&path, OpenOptions::READONLY).expect("open failed");

    let mut buf = [0u8; 256];
    let n = naming::read(fd, &mut buf).expect("read failed");

    let s = core::str::from_utf8(&buf[..n]).unwrap_or("<non-utf8>");
    println!("--- {} ({} bytes) ---", path, n);
    println!("{}", s);
    
    let _ = naming::close(fd);
}

#[unsafe(no_mangle)]
pub fn main() {
    println!("PROC TESTS");
    test_proc_root();
    test_proc_pid(1);
    read_proc_status(1);
    read_proc_ps();
    read_proc_ticks();
    read_proc_meminfo();
    println!("PROC TEST: END");
}
