#![no_std]

use core::panic::PanicInfo;
use linked_list_allocator::LockedHeap;
use io::{print, println};
use syscall::{syscall1, SystemCall};

extern {
    fn main();
}

const HEAP_SIZE: usize = 0x100000;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}!", info);
    thread::exit();
}

#[no_mangle]
extern "C" fn entry() {
    let heap_start = syscall1(SystemCall::MapUserHeap, HEAP_SIZE) as *mut u8;
    unsafe { ALLOCATOR.lock().init(heap_start, HEAP_SIZE); }

    unsafe { main(); }
    thread::exit();
}