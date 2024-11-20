#![no_std]
#![feature(panic_info_message)]

use alloc::string::String;
use concurrent::{process, thread};
use core::fmt::Write;
use core::panic::PanicInfo;
use io::write::log_debug;
use linked_list_allocator::LockedHeap;
use syscall::{syscall1, SystemCall};

extern "C" {
    fn main();
}

extern crate alloc;

const HEAP_SIZE: usize = 0x100000;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(loc) = info.location() {
        log_debug(loc.file());
        let mut s = String::new();
        write!(&mut s, "{} : {}", loc.line(), loc.column()).unwrap();
        log_debug(s.as_str());
    }
    let msg = info.message();
    
    log_debug(msg.as_str().unwrap_or(""));
    
    thread::exit();
}

#[no_mangle]
extern "C" fn entry() {
    let heap_start = syscall1(SystemCall::MapUserHeap, HEAP_SIZE) as *mut u8;
    unsafe {
        ALLOCATOR.lock().init(heap_start, HEAP_SIZE);
    }

    unsafe {
        main();
    }
    process::exit();
}
