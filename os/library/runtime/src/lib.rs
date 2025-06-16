/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Entry function for an application.                              ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 31.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]
extern crate alloc;

pub mod env;

use concurrent::{process, thread};
use core::panic::PanicInfo;
use terminal::{print, println};
use linked_list_allocator::LockedHeap;
use syscall::{syscall, SystemCall};

unsafe extern "C" {
    fn main(argc: isize, argv: *const *const u8) -> isize;
}

const HEAP_SIZE: usize = 0x1000000; // TODO#? 0x100000 is causing allocation error (heap to small for fb, why is it not allocating more???)

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[cfg(not(any(test, feature = "std")))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}!", info);
    thread::exit();
}

#[unsafe(no_mangle)]
extern "C" fn entry() {
    let heap_start: *mut u8;

    let res = syscall(SystemCall::MapUserHeap, &[HEAP_SIZE]);
    match res {
        Ok(hs) => heap_start = hs as *mut u8,
        Err(_) => panic!("Could not create user heap."),
    }

    unsafe {
        ALLOCATOR.lock().init(heap_start, HEAP_SIZE);
    }

    unsafe {
        main(*env::ARGC_PTR as isize, env::ARGV_PTR);
    }
    process::exit();
}
